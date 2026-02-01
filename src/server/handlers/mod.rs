pub mod agent;
pub mod auth;
pub mod availability;
pub mod config;
pub mod inbox;
pub mod proposals;
pub mod webhooks;

use axum::Json;
use serde::Serialize;

/// Health check endpoint
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

/// Standard error response
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl ErrorResponse {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}
