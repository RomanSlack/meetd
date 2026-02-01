use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use super::AppState;
use crate::models::User;

/// Extension for authenticated user
#[derive(Clone)]
pub struct AuthenticatedUser(pub User);

/// API key authentication middleware
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let api_key = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    // Find user by API key
    let user = state
        .db
        .find_user_by_api_key(api_key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Add user to request extensions
    request.extensions_mut().insert(AuthenticatedUser(user));

    Ok(next.run(request).await)
}
