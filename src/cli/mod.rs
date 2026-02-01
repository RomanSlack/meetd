mod accept;
mod auth;
mod availability;
mod config;
mod inbox;
mod propose;

pub use accept::*;
pub use auth::*;
pub use availability::*;
pub use config::*;
pub use inbox::*;
pub use propose::*;

use serde::Serialize;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    pub fn print<T: Serialize + std::fmt::Display>(&self, value: &T) {
        match self {
            OutputFormat::Human => println!("{}", value),
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(value).unwrap());
            }
        }
    }

    pub fn print_json<T: Serialize>(&self, value: &T) {
        match self {
            OutputFormat::Human => {
                println!("{}", serde_json::to_string_pretty(value).unwrap());
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(value).unwrap());
            }
        }
    }
}

/// Common error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {}", self.error)
    }
}

/// Success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

impl std::fmt::Display for SuccessResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Get the API client for making requests to the server
pub fn get_api_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

/// Get the server URL from config or default
pub fn get_server_url() -> String {
    crate::models::LocalConfig::load()
        .ok()
        .and_then(|c| c.server_url)
        .unwrap_or_else(|| crate::DEFAULT_SERVER_URL.to_string())
}

/// Get the API key from local config
pub fn get_api_key() -> anyhow::Result<String> {
    let config = crate::models::LocalConfig::load()?;
    config
        .api_key
        .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'meetd login' first."))
}
