use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::ErrorResponse;
use crate::crypto::generate_webhook_secret;
use crate::models::Visibility;
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;

#[derive(Serialize)]
pub struct ConfigResponse {
    pub visibility: String,
    pub webhook_url: Option<String>,
    pub public_key: String,
}

/// Get current user config
pub async fn get_config(
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        visibility: user.visibility.as_str().to_string(),
        webhook_url: user.webhook_url,
        public_key: user.public_key,
    })
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub visibility: Option<String>,
    pub webhook_url: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateConfigResponse {
    pub visibility: String,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
}

/// Update user config
pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Json(req): Json<UpdateConfigRequest>,
) -> Response {
    let mut new_visibility = user.visibility;
    let mut new_webhook_url = user.webhook_url.clone();
    let mut new_webhook_secret = user.webhook_secret.clone();

    // Update visibility if provided
    if let Some(vis_str) = req.visibility {
        match Visibility::from_str(&vis_str) {
            Some(v) => {
                new_visibility = v;
                if let Err(e) = state.db.update_user_visibility(&user.id, v) {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::new(e.to_string())),
                    )
                        .into_response();
                }
            }
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new(
                        "Invalid visibility. Use: busy_only, masked, or full",
                    )),
                )
                    .into_response();
            }
        }
    }

    // Update webhook if provided
    if let Some(webhook_url) = req.webhook_url {
        let (url, secret) = if webhook_url.is_empty() {
            (None, None)
        } else {
            // Generate new secret when webhook URL changes
            let secret = generate_webhook_secret();
            (Some(webhook_url), Some(secret))
        };

        if let Err(e) = state.db.update_user_webhook(
            &user.id,
            url.as_deref(),
            secret.as_deref(),
        ) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }

        new_webhook_url = url;
        new_webhook_secret = secret;
    }

    Json(UpdateConfigResponse {
        visibility: new_visibility.as_str().to_string(),
        webhook_url: new_webhook_url,
        webhook_secret: new_webhook_secret,
    })
    .into_response()
}
