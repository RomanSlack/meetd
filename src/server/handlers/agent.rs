use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::ErrorResponse;
use crate::calendar::{CalendarProvider, GoogleCalendar};
use crate::crypto::PublicKey;
use crate::models::{
    AcceptProposalResponse, CalendarEvent, Proposal, ProposalStatus, SignedProposal,
};
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;
use crate::webhook::{WebhookClient, WebhookEvent, WebhookEventData, WebhookEventType};

#[derive(Serialize)]
pub struct PubkeyResponse {
    pub email: String,
    pub public_key: String,
}

/// Get a user's public key by email
pub async fn get_pubkey(State(state): State<Arc<AppState>>, Path(email): Path<String>) -> Response {
    match state.db.get_user_by_email(&email) {
        Ok(Some(user)) => Json(PubkeyResponse {
            email: user.email,
            public_key: user.public_key,
        })
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("User not found")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct ReceiveProposalRequest {
    pub signed_proposal: String,
    #[serde(default)]
    pub action: Option<String>, // "accept" to auto-accept
}

#[derive(Serialize)]
pub struct ReceiveProposalResponse {
    pub proposal_id: String,
    pub status: String,
    pub event: Option<CalendarEvent>,
}

/// Receive a signed proposal from another agent
pub async fn receive_proposal(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Json(req): Json<ReceiveProposalRequest>,
) -> Response {
    // Decode base64
    let json_bytes = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &req.signed_proposal,
    ) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(format!("Invalid base64: {}", e))),
            )
                .into_response();
        }
    };

    // Parse JSON
    let signed: SignedProposal = match serde_json::from_slice(&json_bytes) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(format!("Invalid JSON: {}", e))),
            )
                .into_response();
        }
    };

    // Verify the proposal is for this user
    if signed.to != user.email {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Proposal is not addressed to you")),
        )
            .into_response();
    }

    // Verify signature
    let pub_key = match PublicKey::from_base64(&signed.from_pubkey) {
        Ok(pk) => pk,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(format!("Invalid public key: {}", e))),
            )
                .into_response();
        }
    };

    match pub_key.verify_proposal(&signed) {
        Ok(false) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("Invalid signature")),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(format!("Verification error: {}", e))),
            )
                .into_response();
        }
        Ok(true) => {}
    }

    // Check expiration
    if signed.expires_at < Utc::now() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Proposal has expired")),
        )
            .into_response();
    }

    // Check nonce hasn't been used
    if let Ok(true) = state.db.is_nonce_used(&signed.nonce) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Nonce already used (replay attack?)")),
        )
            .into_response();
    }

    // Mark nonce as used
    if let Err(e) = state.db.use_nonce(&signed.nonce) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    // Find or create sender user ID
    let from_user_id = if let Ok(Some(sender)) = state.db.get_user_by_email(&signed.from) {
        sender.id
    } else {
        // External sender - use email as ID
        signed.from.clone()
    };

    // Create proposal in database
    let proposal_id = format!(
        "prop_{}",
        &Uuid::new_v4().to_string().replace('-', "")[..12]
    );
    let proposal = Proposal {
        id: proposal_id.clone(),
        from_user_id: from_user_id.clone(),
        to_email: user.email.clone(),
        slot_start: signed.slot.start,
        duration_minutes: signed.slot.duration_minutes,
        title: signed.title.clone(),
        description: signed.description.clone(),
        nonce: signed.nonce.clone(),
        expires_at: signed.expires_at,
        signature: signed.signature.clone(),
        status: ProposalStatus::Pending,
        created_at: Utc::now().timestamp(),
    };

    if let Err(e) = state.db.create_proposal(&proposal) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!(
                "Failed to create proposal: {}",
                e
            ))),
        )
            .into_response();
    }

    // If action is "accept", auto-accept the proposal
    if req.action.as_deref() == Some("accept") {
        let end_time = proposal.slot_start + Duration::minutes(proposal.duration_minutes as i64);
        let mut calendar_link = None;

        // Create calendar event
        if let Some(ref token) = user.google_refresh_token {
            if let Ok(cal) =
                GoogleCalendar::new(&state.google_client_id, &state.google_client_secret, token)
                    .await
            {
                if let Ok(event) = cal
                    .create_event(
                        proposal.title.as_deref().unwrap_or("Meeting"),
                        proposal.description.as_deref(),
                        proposal.slot_start,
                        end_time,
                        Some(&signed.from),
                    )
                    .await
                {
                    calendar_link = event.html_link;
                }
            }
        }

        // Update status
        if let Err(e) = state
            .db
            .update_proposal_status(&proposal_id, ProposalStatus::Accepted)
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string())),
            )
                .into_response();
        }

        // Send webhook to sender if they're in our system
        if let Ok(Some(sender)) = state.db.get_user_by_email(&signed.from) {
            if let (Some(webhook_url), Some(webhook_secret)) =
                (sender.webhook_url, sender.webhook_secret)
            {
                let event = WebhookEvent::new(
                    WebhookEventType::ProposalAccepted,
                    WebhookEventData::proposal_accepted(
                        proposal_id.clone(),
                        user.email.clone(),
                        calendar_link.clone(),
                    ),
                );

                let webhook_client = WebhookClient::new();
                tokio::spawn(async move {
                    if let Err(e) = webhook_client
                        .deliver(&webhook_url, &webhook_secret, &event)
                        .await
                    {
                        tracing::warn!("Failed to deliver webhook: {}", e);
                    }
                });
            }
        }

        return Json(AcceptProposalResponse {
            status: "accepted".to_string(),
            event: Some(CalendarEvent {
                title: proposal.title.unwrap_or_else(|| "Meeting".to_string()),
                start: proposal.slot_start,
                end: end_time,
                calendar_link,
            }),
        })
        .into_response();
    }

    Json(ReceiveProposalResponse {
        proposal_id,
        status: "pending".to_string(),
        event: None,
    })
    .into_response()
}
