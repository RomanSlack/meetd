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
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;
use crate::webhook::{WebhookClient, WebhookEvent, WebhookEventData, WebhookEventType};

#[derive(Deserialize)]
pub struct RegisterWebhookRequest {
    pub url: String,
}

#[derive(Serialize)]
pub struct RegisterWebhookResponse {
    pub url: String,
    pub secret: String,
}

/// Register or update webhook URL
pub async fn register_webhook(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Json(req): Json<RegisterWebhookRequest>,
) -> Response {
    // Validate URL
    if let Err(e) = url::Url::parse(&req.url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(format!("Invalid URL: {}", e))),
        )
            .into_response();
    }

    // Generate new secret
    let secret = generate_webhook_secret();

    if let Err(e) = state
        .db
        .update_user_webhook(&user.id, Some(&req.url), Some(&secret))
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    Json(RegisterWebhookResponse {
        url: req.url,
        secret,
    })
    .into_response()
}

/// Remove webhook
pub async fn remove_webhook(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
) -> Response {
    if let Err(e) = state.db.update_user_webhook(&user.id, None, None) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    Json(serde_json::json!({ "status": "removed" })).into_response()
}

#[derive(Serialize)]
pub struct TestWebhookResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// Test webhook delivery
pub async fn test_webhook(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
) -> Response {
    // Get current user with webhook config
    let user = match state.db.get_user(&user.id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }
    };

    let (webhook_url, webhook_secret) = match (user.webhook_url, user.webhook_secret) {
        (Some(url), Some(secret)) => (url, secret),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("No webhook configured")),
            )
                .into_response();
        }
    };

    // Create test event
    let event = WebhookEvent::new(
        WebhookEventType::ProposalReceived,
        WebhookEventData {
            proposal_id: "test_proposal".to_string(),
            from: "test@meetd.example.com".to_string(),
            from_pubkey: None,
            slot: None,
            title: Some("Test Webhook".to_string()),
            expires_at: None,
            signature: None,
            calendar_link: None,
        },
    );

    let client = WebhookClient::new();
    match client.deliver(&webhook_url, &webhook_secret, &event).await {
        Ok(()) => Json(TestWebhookResponse {
            success: true,
            error: None,
        })
        .into_response(),
        Err(e) => Json(TestWebhookResponse {
            success: false,
            error: Some(e.to_string()),
        })
        .into_response(),
    }
}
