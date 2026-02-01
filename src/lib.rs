pub mod calendar;
pub mod cli;
pub mod crypto;
pub mod db;
pub mod models;
pub mod server;
pub mod webhook;

pub use models::*;

/// Default server URL for the meetd API
pub const DEFAULT_SERVER_URL: &str = "https://meetd.fly.dev";

/// API version prefix
pub const API_VERSION: &str = "v1";

/// Application name for OAuth
pub const APP_NAME: &str = "meetd";
