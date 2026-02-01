use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use super::handlers;
use super::middleware::auth_middleware;
use super::AppState;

/// Create the API router
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/google", get(handlers::auth::google_redirect))
        .route("/auth/callback", get(handlers::auth::google_callback))
        .route("/v1/agent/pubkey/:email", get(handlers::agent::get_pubkey));

    // Protected routes (require API key)
    let protected_routes = Router::new()
        // Config
        .route("/v1/config", get(handlers::config::get_config))
        .route("/v1/config", patch(handlers::config::update_config))
        // Availability
        .route(
            "/v1/availability",
            post(handlers::availability::query_availability),
        )
        // Proposals
        .route("/v1/proposals", post(handlers::proposals::create_proposal))
        .route(
            "/v1/proposals/sent",
            get(handlers::proposals::get_sent_proposals),
        )
        .route("/v1/proposals/:id", get(handlers::proposals::get_proposal))
        .route(
            "/v1/proposals/:id/accept",
            post(handlers::proposals::accept_proposal),
        )
        .route(
            "/v1/proposals/:id/decline",
            post(handlers::proposals::decline_proposal),
        )
        .route(
            "/v1/proposals/verify",
            post(handlers::proposals::verify_proposal),
        )
        // Inbox
        .route("/v1/inbox", get(handlers::inbox::list_inbox))
        .route("/v1/inbox/stream", get(handlers::inbox::stream_inbox))
        // Agent-to-agent
        .route("/v1/agent/inbox", post(handlers::agent::receive_proposal))
        // Webhooks
        .route("/v1/webhooks", post(handlers::webhooks::register_webhook))
        .route("/v1/webhooks", delete(handlers::webhooks::remove_webhook))
        .route("/v1/webhooks/test", post(handlers::webhooks::test_webhook))
        // API key rotation
        .route("/auth/key/rotate", post(handlers::auth::rotate_api_key))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
