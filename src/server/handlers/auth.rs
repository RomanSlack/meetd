use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::ErrorResponse;
use crate::crypto::{generate_api_key, hash_api_key, Keypair};
use crate::models::{User, Visibility};
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub api_key: String,
}

/// Register a new user (called after OAuth flow)
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Response {
    match register_user_internal(&state, &req.email, &req.refresh_token).await {
        Ok((user_id, api_key)) => Json(RegisterResponse { user_id, api_key }).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(error)),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct GoogleRedirectQuery {
    /// CLI callback URL for redirecting after auth (e.g., http://localhost:8899)
    pub cli_callback: Option<String>,
}

/// Redirect to Google OAuth
pub async fn google_redirect(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GoogleRedirectQuery>,
) -> Response {
    if state.google_client_id.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Google OAuth not configured")),
        )
            .into_response();
    }

    let scopes = [
        "https://www.googleapis.com/auth/calendar.readonly",
        "https://www.googleapis.com/auth/calendar.events",
        "https://www.googleapis.com/auth/userinfo.email",
    ]
    .join("%20");

    let redirect_uri = format!("{}/auth/callback", state.server_url);

    // Encode CLI callback URL in state parameter if provided
    let state_param = query
        .cli_callback
        .map(|cb| urlencoding::encode(&cb).to_string())
        .unwrap_or_default();

    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope={}&\
         access_type=offline&\
         prompt=consent&\
         state={}",
        state.google_client_id,
        urlencoding::encode(&redirect_uri),
        scopes,
        state_param
    );

    Redirect::temporary(&url).into_response()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
    /// CLI callback URL to redirect to after auth (for CLI login flow)
    pub state: Option<String>,
}

/// Handle Google OAuth callback
pub async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    // Decode CLI callback URL from state parameter
    let cli_callback = query
        .state
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string());

    if let Some(error) = query.error {
        if let Some(callback) = cli_callback {
            return Redirect::temporary(&format!(
                "{}?error={}",
                callback,
                urlencoding::encode(&error)
            ))
            .into_response();
        }
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse::new(error))).into_response();
    }

    let code = match query.code {
        Some(c) => c,
        None => {
            let error = "Missing authorization code";
            if let Some(callback) = cli_callback {
                return Redirect::temporary(&format!(
                    "{}?error={}",
                    callback,
                    urlencoding::encode(error)
                ))
                .into_response();
            }
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse::new(error))).into_response();
        }
    };

    // Exchange code for tokens
    let redirect_uri = format!("{}/auth/callback", state.server_url);
    let client = reqwest::Client::new();
    let resp = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", &state.google_client_id),
            ("client_secret", &state.google_client_secret),
            ("redirect_uri", &redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let error = format!("Token exchange failed: {}", e);
            if let Some(callback) = cli_callback {
                return Redirect::temporary(&format!(
                    "{}?error={}",
                    callback,
                    urlencoding::encode(&error)
                ))
                .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(error)),
            )
                .into_response();
        }
    };

    if !resp.status().is_success() {
        let error = format!(
            "Token exchange failed: {}",
            resp.text().await.unwrap_or_default()
        );
        if let Some(callback) = cli_callback {
            return Redirect::temporary(&format!(
                "{}?error={}",
                callback,
                urlencoding::encode(&error)
            ))
            .into_response();
        }
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(error)),
        )
            .into_response();
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
    }

    let tokens: TokenResponse = match resp.json().await {
        Ok(t) => t,
        Err(e) => {
            let error = format!("Failed to parse tokens: {}", e);
            if let Some(callback) = cli_callback {
                return Redirect::temporary(&format!(
                    "{}?error={}",
                    callback,
                    urlencoding::encode(&error)
                ))
                .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(error)),
            )
                .into_response();
        }
    };

    // Get user email
    let email = match crate::calendar::GoogleCalendar::get_user_email(&tokens.access_token).await {
        Ok(e) => e,
        Err(e) => {
            let error = format!("Failed to get user email: {}", e);
            if let Some(callback) = cli_callback {
                return Redirect::temporary(&format!(
                    "{}?error={}",
                    callback,
                    urlencoding::encode(&error)
                ))
                .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(error)),
            )
                .into_response();
        }
    };

    let refresh_token = tokens.refresh_token.unwrap_or_default();

    // Create/update user
    let reg_result = register_user_internal(&state, &email, &refresh_token).await;

    match reg_result {
        Ok((user_id, api_key)) => {
            if let Some(callback) = cli_callback {
                // Redirect back to CLI with credentials
                return Redirect::temporary(&format!(
                    "{}?api_key={}&email={}&user_id={}",
                    callback,
                    urlencoding::encode(&api_key),
                    urlencoding::encode(&email),
                    urlencoding::encode(&user_id)
                ))
                .into_response();
            }
            // No CLI callback - show web page with API key
            axum::response::Html(format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>meetd - Login Successful</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        * {{ box-sizing: border-box; }}
        body {{ font-family: -apple-system, system-ui, sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; padding: 40px 20px; margin: 0; }}
        .container {{ background: white; border-radius: 16px; padding: 40px; max-width: 560px; margin: 0 auto; box-shadow: 0 20px 60px rgba(0,0,0,0.3); }}
        .header {{ text-align: center; margin-bottom: 32px; }}
        .checkmark {{ width: 64px; height: 64px; background: #22c55e; border-radius: 50%; display: flex; align-items: center; justify-content: center; margin: 0 auto 16px; }}
        .checkmark svg {{ width: 32px; height: 32px; stroke: white; stroke-width: 3; fill: none; }}
        h1 {{ color: #1a1a1a; margin: 0 0 8px; font-size: 24px; }}
        .email {{ color: #666; margin: 0; }}
        .field {{ margin-bottom: 20px; }}
        .label {{ font-size: 11px; color: #888; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 8px; font-weight: 600; }}
        .value {{ background: #f5f5f5; padding: 14px 16px; border-radius: 8px; font-family: 'SF Mono', Monaco, monospace; font-size: 14px; word-break: break-all; position: relative; border: 1px solid #e5e5e5; }}
        .value.has-btn {{ padding-right: 70px; }}
        .copy-btn {{ position: absolute; right: 8px; top: 50%; transform: translateY(-50%); background: #22c55e; color: white; border: none; padding: 8px 14px; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: 600; transition: background 0.2s; }}
        .copy-btn:hover {{ background: #16a34a; }}
        .divider {{ height: 1px; background: #e5e5e5; margin: 28px 0; }}
        .instructions {{ background: #fafafa; border-radius: 12px; padding: 20px; }}
        .instructions h3 {{ margin: 0 0 16px; font-size: 14px; color: #333; }}
        .step {{ margin-bottom: 16px; }}
        .step:last-child {{ margin-bottom: 0; }}
        .step-label {{ font-size: 12px; color: #666; margin-bottom: 8px; }}
        .code-block {{ background: #1a1a1a; color: #e5e5e5; padding: 12px 14px; border-radius: 8px; font-family: 'SF Mono', Monaco, monospace; font-size: 12px; overflow-x: auto; white-space: nowrap; }}
        .code-block::-webkit-scrollbar {{ height: 6px; }}
        .code-block::-webkit-scrollbar-track {{ background: #333; border-radius: 3px; }}
        .code-block::-webkit-scrollbar-thumb {{ background: #666; border-radius: 3px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div class="checkmark">
                <svg viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12"></polyline></svg>
            </div>
            <h1>Login Successful</h1>
            <p class="email">{email}</p>
        </div>

        <div class="field">
            <div class="label">Your API Key</div>
            <div class="value has-btn">
                {api_key}
                <button class="copy-btn" onclick="navigator.clipboard.writeText('{api_key}'); this.textContent='Copied!'; setTimeout(() => this.textContent='Copy', 2000)">Copy</button>
            </div>
        </div>

        <div class="field">
            <div class="label">User ID</div>
            <div class="value">{user_id}</div>
        </div>

        <div class="divider"></div>

        <div class="instructions">
            <h3>Next Steps</h3>
            <div class="step">
                <div class="step-label">Use the REST API:</div>
                <div class="code-block">curl https://meetd.fly.dev/v1/inbox -H "Authorization: Bearer {api_key}"</div>
            </div>
            <div class="step">
                <div class="step-label">Or save config for CLI:</div>
                <div class="code-block">mkdir -p ~/.config/meetd && echo '{{"api_key":"{api_key}","server_url":"https://meetd.fly.dev","email":"{email}","user_id":"{user_id}"}}' > ~/.config/meetd/config.json</div>
            </div>
        </div>
    </div>
</body>
</html>"#,
                email = email,
                api_key = api_key,
                user_id = user_id
            ))
            .into_response()
        }
        Err(error) => {
            if let Some(callback) = cli_callback {
                return Redirect::temporary(&format!(
                    "{}?error={}",
                    callback,
                    urlencoding::encode(&error)
                ))
                .into_response();
            }
            // No CLI callback - show error page
            axum::response::Html(format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>meetd - Login Failed</title>
    <style>
        body {{ font-family: -apple-system, system-ui, sans-serif; background: #f5f5f5; padding: 40px; text-align: center; }}
        .container {{ background: white; border-radius: 12px; padding: 40px; max-width: 500px; margin: 0 auto; box-shadow: 0 4px 20px rgba(0,0,0,0.1); }}
        h1 {{ color: #ef4444; }}
        .error {{ color: #666; margin-bottom: 24px; }}
        a {{ color: #22c55e; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Login Failed</h1>
        <p class="error">{error}</p>
        <p><a href="/auth/google">Try again</a></p>
    </div>
</body>
</html>"#,
                error = error
            ))
            .into_response()
        }
    }
}

/// Internal function to register/update a user, returns (user_id, api_key) or error
async fn register_user_internal(
    state: &AppState,
    email: &str,
    refresh_token: &str,
) -> Result<(String, String), String> {
    // Check if user already exists
    if let Ok(Some(existing)) = state.db.get_user_by_email(email) {
        // Generate new API key for existing user
        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).map_err(|e| e.to_string())?;

        state
            .db
            .update_user_api_key_hash(&existing.id, &api_key_hash)
            .map_err(|e| e.to_string())?;

        state
            .db
            .update_user_refresh_token(&existing.id, Some(refresh_token))
            .map_err(|e| e.to_string())?;

        return Ok((existing.id, api_key));
    }

    // Generate keypair for new user
    let keypair = Keypair::generate().map_err(|e| e.to_string())?;

    // Generate API key
    let api_key = generate_api_key();
    let api_key_hash = hash_api_key(&api_key).map_err(|e| e.to_string())?;

    let user_id = Uuid::new_v4().to_string();
    let user = User {
        id: user_id.clone(),
        email: email.to_string(),
        google_refresh_token: Some(refresh_token.to_string()),
        public_key: keypair.public_key_base64(),
        private_key: keypair.private_key_base64(),
        api_key_hash,
        visibility: Visibility::BusyOnly,
        webhook_url: None,
        webhook_secret: None,
        created_at: Utc::now().timestamp(),
    };

    state.db.create_user(&user).map_err(|e| e.to_string())?;

    Ok((user_id, api_key))
}

#[derive(Serialize)]
pub struct RotateKeyResponse {
    pub api_key: String,
}

/// Rotate API key
pub async fn rotate_api_key(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
) -> Response {
    let api_key = generate_api_key();
    let api_key_hash = match hash_api_key(&api_key) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response()
        }
    };

    if let Err(e) = state.db.update_user_api_key_hash(&user.id, &api_key_hash) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    Json(RotateKeyResponse { api_key }).into_response()
}
