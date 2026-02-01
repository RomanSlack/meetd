mod handlers;
mod middleware;
mod routes;

pub use routes::create_router;

use anyhow::Result;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::db::Database;

/// Shared application state
pub struct AppState {
    pub db: Database,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub server_url: String,
}

impl AppState {
    pub fn new(
        db: Database,
        google_client_id: String,
        google_client_secret: String,
        server_url: String,
    ) -> Self {
        Self {
            db,
            google_client_id,
            google_client_secret,
            server_url,
        }
    }
}

/// Run the API server
pub async fn run_server(
    addr: SocketAddr,
    db_path: &str,
    server_url: &str,
) -> Result<()> {
    // Get Google OAuth credentials
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| "".to_string());
    let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .unwrap_or_else(|_| "".to_string());

    // Open database
    let db = Database::open(db_path)?;

    // Create app state
    let state = Arc::new(AppState::new(
        db,
        google_client_id,
        google_client_secret,
        server_url.to_string(),
    ));

    // Create router
    let app = create_router(state);

    tracing::info!("Server listening on {}", addr);

    // Run server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
