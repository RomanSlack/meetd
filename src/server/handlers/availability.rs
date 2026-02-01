use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

use super::ErrorResponse;
use crate::calendar::{intersect_availability, score_slot, CalendarProvider, GoogleCalendar};
use crate::models::{AvailabilityRequest, AvailableSlot};
use crate::server::middleware::AuthenticatedUser;
use crate::server::AppState;

#[derive(Serialize)]
pub struct AvailabilityResponse {
    pub slots: Vec<AvailableSlot>,
}

/// Query availability for a meeting
pub async fn query_availability(
    State(state): State<Arc<AppState>>,
    Extension(AuthenticatedUser(user)): Extension<AuthenticatedUser>,
    Json(req): Json<AvailabilityRequest>,
) -> Response {
    // Get requester's busy periods
    let requester_busy = if let Some(ref token) = user.google_refresh_token {
        match GoogleCalendar::new(
            &state.google_client_id,
            &state.google_client_secret,
            token,
        )
        .await
        {
            Ok(cal) => match cal.get_busy_periods(req.window_start, req.window_end).await {
                Ok(busy) => busy,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::new(format!(
                            "Failed to get your calendar: {}",
                            e
                        ))),
                    )
                        .into_response();
                }
            },
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(format!(
                        "Failed to connect to calendar: {}",
                        e
                    ))),
                )
                    .into_response();
            }
        }
    } else {
        vec![]
    };

    // Get target user's busy periods (if they exist in our system)
    let target_busy = if let Ok(Some(target)) = state.db.get_user_by_email(&req.with_email) {
        if let Some(ref token) = target.google_refresh_token {
            match GoogleCalendar::new(
                &state.google_client_id,
                &state.google_client_secret,
                token,
            )
            .await
            {
                Ok(cal) => cal
                    .get_busy_periods(req.window_start, req.window_end)
                    .await
                    .unwrap_or_default(),
                Err(_) => vec![],
            }
        } else {
            vec![]
        }
    } else {
        // Target not in system - only use requester's availability
        vec![]
    };

    // Find intersection of availability
    let available = intersect_availability(
        &requester_busy,
        &target_busy,
        req.window_start,
        req.window_end,
        req.duration_minutes,
    );

    // Score and sort slots
    let now = Utc::now();
    let mut scored_slots: Vec<AvailableSlot> = available
        .into_iter()
        .map(|slot| {
            let score = score_slot(&slot, now);
            AvailableSlot {
                start: slot.start,
                end: slot.end,
                score,
            }
        })
        .collect();

    // Sort by score descending
    scored_slots.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Limit to top 20 slots
    scored_slots.truncate(20);

    Json(AvailabilityResponse { slots: scored_slots }).into_response()
}
