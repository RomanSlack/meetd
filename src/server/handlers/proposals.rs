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
use crate::crypto::{Keypair, PublicKey};
use crate::models::{
    AcceptProposalResponse, CalendarEvent, CreateProposalRequest, CreateProposalResponse,
    InboxProposal, Proposal, ProposalSlot, ProposalStatus, SignedProposal,
};
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;
use crate::webhook::{WebhookClient, WebhookEvent, WebhookEventData, WebhookEventType};

/// Create a new proposal
pub async fn create_proposal(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Json(req): Json<CreateProposalRequest>,
) -> Response {
    let proposal_id = format!(
        "prop_{}",
        &Uuid::new_v4().to_string().replace('-', "")[..12]
    );
    let nonce = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(7);

    // Load user's keypair
    let keypair = match Keypair::from_private_key_base64(&user.private_key) {
        Ok(kp) => kp,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(format!("Failed to load keypair: {}", e))),
            )
                .into_response();
        }
    };

    // Create signed proposal
    let mut signed = SignedProposal {
        version: 1,
        from: user.email.clone(),
        from_pubkey: user.public_key.clone(),
        to: req.to_email.clone(),
        slot: ProposalSlot {
            start: req.slot_start,
            duration_minutes: req.duration_minutes,
        },
        title: req.title.clone(),
        description: req.description.clone(),
        nonce: nonce.clone(),
        expires_at,
        signature: String::new(),
    };

    keypair.sign_proposal(&mut signed);

    // Store proposal in database
    let proposal = Proposal {
        id: proposal_id.clone(),
        from_user_id: user.id.clone(),
        to_email: req.to_email.clone(),
        slot_start: req.slot_start,
        duration_minutes: req.duration_minutes,
        title: req.title,
        description: req.description,
        nonce,
        expires_at,
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

    // Send webhook if recipient has one configured
    if let Ok(Some(recipient)) = state.db.get_user_by_email(&req.to_email) {
        if let (Some(webhook_url), Some(webhook_secret)) =
            (recipient.webhook_url, recipient.webhook_secret)
        {
            let event = WebhookEvent::new(
                WebhookEventType::ProposalReceived,
                WebhookEventData::proposal_received(
                    proposal_id.clone(),
                    user.email.clone(),
                    user.public_key.clone(),
                    signed.slot.clone(),
                    signed.title.clone(),
                    expires_at,
                    signed.signature.clone(),
                ),
            );

            let webhook_client = WebhookClient::new();
            // Fire and forget - don't block on webhook delivery
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

    // Serialize signed proposal
    let signed_json = serde_json::to_string(&signed).unwrap();
    let signed_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        signed_json.as_bytes(),
    );

    Json(CreateProposalResponse {
        proposal_id,
        signed_proposal: signed_b64,
        accept_link: format!("{}/accept/{}", state.server_url, proposal.id),
    })
    .into_response()
}

/// Get a proposal by ID
pub async fn get_proposal(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Response {
    match state.db.get_proposal(&id) {
        Ok(Some(proposal)) => {
            // Check if user is sender or recipient
            if proposal.from_user_id != user.id && proposal.to_email != user.email {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse::new("Not authorized to view this proposal")),
                )
                    .into_response();
            }

            Json(InboxProposal::from(proposal)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("Proposal not found")),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response(),
    }
}

/// Get sent proposals
pub async fn get_sent_proposals(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
) -> Response {
    match state.db.get_proposals_from_user(&user.id) {
        Ok(proposals) => {
            let inbox: Vec<InboxProposal> =
                proposals.into_iter().map(InboxProposal::from).collect();
            Json(serde_json::json!({ "proposals": inbox })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response(),
    }
}

/// Accept a proposal
pub async fn accept_proposal(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Response {
    // Get proposal
    let proposal = match state.db.get_proposal(&id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("Proposal not found")),
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

    // Check if user is the recipient
    if proposal.to_email != user.email {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Not authorized to accept this proposal")),
        )
            .into_response();
    }

    // Check if proposal is still pending
    if proposal.status != ProposalStatus::Pending {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(format!(
                "Proposal is already {}",
                proposal.status
            ))),
        )
            .into_response();
    }

    // Check if proposal has expired
    if proposal.expires_at < Utc::now() {
        let _ = state
            .db
            .update_proposal_status(&id, ProposalStatus::Expired);
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Proposal has expired")),
        )
            .into_response();
    }

    // Create calendar event
    let end_time = proposal.slot_start + Duration::minutes(proposal.duration_minutes as i64);
    let mut calendar_link = None;

    if let Some(ref token) = user.google_refresh_token {
        if let Ok(cal) =
            GoogleCalendar::new(&state.google_client_id, &state.google_client_secret, token).await
        {
            // Get sender email
            let sender_email = if let Ok(Some(sender)) = state.db.get_user(&proposal.from_user_id) {
                Some(sender.email)
            } else {
                None
            };

            if let Ok(event) = cal
                .create_event(
                    proposal.title.as_deref().unwrap_or("Meeting"),
                    proposal.description.as_deref(),
                    proposal.slot_start,
                    end_time,
                    sender_email.as_deref(),
                )
                .await
            {
                calendar_link = event.html_link;
            }
        }
    }

    // Update proposal status
    if let Err(e) = state
        .db
        .update_proposal_status(&id, ProposalStatus::Accepted)
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    // Send webhook to sender
    if let Ok(Some(sender)) = state.db.get_user(&proposal.from_user_id) {
        if let (Some(webhook_url), Some(webhook_secret)) =
            (sender.webhook_url, sender.webhook_secret)
        {
            let event = WebhookEvent::new(
                WebhookEventType::ProposalAccepted,
                WebhookEventData::proposal_accepted(
                    id.clone(),
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

    Json(AcceptProposalResponse {
        status: "accepted".to_string(),
        event: Some(CalendarEvent {
            title: proposal.title.unwrap_or_else(|| "Meeting".to_string()),
            start: proposal.slot_start,
            end: end_time,
            calendar_link,
        }),
    })
    .into_response()
}

/// Decline a proposal
pub async fn decline_proposal(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Response {
    // Get proposal
    let proposal = match state.db.get_proposal(&id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("Proposal not found")),
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

    // Check if user is the recipient
    if proposal.to_email != user.email {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "Not authorized to decline this proposal",
            )),
        )
            .into_response();
    }

    // Update status
    if let Err(e) = state
        .db
        .update_proposal_status(&id, ProposalStatus::Declined)
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response();
    }

    // Send webhook to sender
    if let Ok(Some(sender)) = state.db.get_user(&proposal.from_user_id) {
        if let (Some(webhook_url), Some(webhook_secret)) =
            (sender.webhook_url, sender.webhook_secret)
        {
            let event = WebhookEvent::new(
                WebhookEventType::ProposalDeclined,
                WebhookEventData::proposal_declined(id.clone(), user.email.clone()),
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

    Json(serde_json::json!({ "status": "declined" })).into_response()
}

#[derive(Deserialize)]
pub struct VerifyProposalRequest {
    pub signed_proposal: String,
}

#[derive(Serialize)]
pub struct VerifyProposalResponse {
    pub valid: bool,
    pub proposal: Option<SignedProposal>,
    pub error: Option<String>,
}

/// Verify a signed proposal
pub async fn verify_proposal(Json(req): Json<VerifyProposalRequest>) -> Response {
    // Decode base64
    let json_bytes = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &req.signed_proposal,
    ) {
        Ok(b) => b,
        Err(e) => {
            return Json(VerifyProposalResponse {
                valid: false,
                proposal: None,
                error: Some(format!("Invalid base64: {}", e)),
            })
            .into_response();
        }
    };

    // Parse JSON
    let proposal: SignedProposal = match serde_json::from_slice(&json_bytes) {
        Ok(p) => p,
        Err(e) => {
            return Json(VerifyProposalResponse {
                valid: false,
                proposal: None,
                error: Some(format!("Invalid JSON: {}", e)),
            })
            .into_response();
        }
    };

    // Verify signature
    let pub_key = match PublicKey::from_base64(&proposal.from_pubkey) {
        Ok(pk) => pk,
        Err(e) => {
            return Json(VerifyProposalResponse {
                valid: false,
                proposal: Some(proposal),
                error: Some(format!("Invalid public key: {}", e)),
            })
            .into_response();
        }
    };

    match pub_key.verify_proposal(&proposal) {
        Ok(true) => Json(VerifyProposalResponse {
            valid: true,
            proposal: Some(proposal),
            error: None,
        })
        .into_response(),
        Ok(false) => Json(VerifyProposalResponse {
            valid: false,
            proposal: Some(proposal),
            error: Some("Invalid signature".to_string()),
        })
        .into_response(),
        Err(e) => Json(VerifyProposalResponse {
            valid: false,
            proposal: Some(proposal),
            error: Some(format!("Verification error: {}", e)),
        })
        .into_response(),
    }
}
