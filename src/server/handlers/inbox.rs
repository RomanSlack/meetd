use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::stream::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use super::ErrorResponse;
use crate::models::{InboxProposal, ProposalStatus};
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}

/// List inbox proposals
pub async fn list_inbox(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Query(query): Query<InboxQuery>,
) -> Response {
    // Expire old proposals first
    let _ = state.db.expire_old_proposals();

    let status = query
        .status
        .as_deref()
        .and_then(ProposalStatus::from_str);

    match state.db.get_proposals_for_email(&user.email, status) {
        Ok(proposals) => {
            // Get sender emails for each proposal
            let mut inbox: Vec<InboxProposal> = Vec::new();
            for p in proposals {
                let from_email = if let Ok(Some(sender)) = state.db.get_user(&p.from_user_id) {
                    sender.email
                } else {
                    p.from_user_id.clone()
                };

                inbox.push(InboxProposal {
                    id: p.id,
                    from: from_email,
                    slot: crate::models::ProposalSlot {
                        start: p.slot_start,
                        duration_minutes: p.duration_minutes,
                    },
                    title: p.title,
                    expires_at: p.expires_at,
                    status: p.status,
                });
            }
            Json(serde_json::json!({ "proposals": inbox })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(e.to_string())),
        )
            .into_response(),
    }
}

/// Stream inbox updates via SSE
pub async fn stream_inbox(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let email = user.email.clone();
    let db = state.db.clone();

    // Keep track of last seen proposal IDs
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Initialize with current proposals
    if let Ok(proposals) = db.get_proposals_for_email(&email, Some(ProposalStatus::Pending)) {
        for p in proposals {
            seen_ids.insert(p.id);
        }
    }

    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Check for new proposals
            if let Ok(proposals) = db.get_proposals_for_email(&email, Some(ProposalStatus::Pending)) {
                for p in proposals {
                    if !seen_ids.contains(&p.id) {
                        seen_ids.insert(p.id.clone());

                        // Get sender email
                        let from_email = if let Ok(Some(sender)) = db.get_user(&p.from_user_id) {
                            sender.email
                        } else {
                            p.from_user_id.clone()
                        };

                        let inbox_proposal = InboxProposal {
                            id: p.id,
                            from: from_email,
                            slot: crate::models::ProposalSlot {
                                start: p.slot_start,
                                duration_minutes: p.duration_minutes,
                            },
                            title: p.title,
                            expires_at: p.expires_at,
                            status: p.status,
                        };

                        if let Ok(json) = serde_json::to_string(&inbox_proposal) {
                            yield Ok(Event::default().data(json));
                        }
                    }
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
