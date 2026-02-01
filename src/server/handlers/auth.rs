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
    // Check if user already exists
    if let Ok(Some(existing)) = state.db.get_user_by_email(&req.email) {
        // Generate new API key for existing user
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

        if let Err(e) = state.db.update_user_api_key_hash(&existing.id, &api_key_hash) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }

        // Update refresh token
        if let Err(e) = state.db.update_user_refresh_token(&existing.id, Some(&req.refresh_token)) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }

        return Json(RegisterResponse {
            user_id: existing.id,
            api_key,
        })
        .into_response();
    }

    // Generate keypair
    let keypair = match Keypair::generate() {
        Ok(kp) => kp,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response()
        }
    };

    // Generate API key
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

    let user_id = Uuid::new_v4().to_string();
    let user = User {
        id: user_id.clone(),
        email: req.email,
        google_refresh_token: Some(req.refresh_token),
        public_key: keypair.public_key_base64(),
        private_key: keypair.private_key_base64(),
        api_key_hash,
        visibility: Visibility::BusyOnly,
        webhook_url: None,
        webhook_secret: None,
        created_at: Utc::now().timestamp(),
    };

    if let Err(e) = state.db.create_user(&user) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    Json(RegisterResponse { user_id, api_key }).into_response()
}

/// Redirect to Google OAuth
pub async fn google_redirect(State(state): State<Arc<AppState>>) -> Response {
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
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope={}&\
         access_type=offline&\
         prompt=consent",
        state.google_client_id,
        urlencoding::encode(&redirect_uri),
        scopes
    );

    Redirect::temporary(&url).into_response()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
}

/// Handle Google OAuth callback
pub async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if let Some(error) = query.error {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse::new(error))).into_response();
    }

    let code = match query.code {
        Some(c) => c,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("Missing authorization code")),
            )
                .into_response()
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(format!("Token exchange failed: {}", e))),
            )
                .into_response()
        }
    };

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!("Token exchange failed: {}", error))),
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(format!("Failed to parse tokens: {}", e))),
            )
                .into_response()
        }
    };

    // Get user email
    let email = match crate::calendar::GoogleCalendar::get_user_email(&tokens.access_token).await {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(format!("Failed to get user email: {}", e))),
            )
                .into_response()
        }
    };

    let refresh_token = tokens.refresh_token.unwrap_or_default();

    // Create/update user (reuse register logic)
    let reg_req = RegisterRequest {
        email,
        refresh_token,
    };

    // Call register internally
    register(State(state), Json(reg_req)).await
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
