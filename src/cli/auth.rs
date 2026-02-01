use anyhow::{Context, Result};
use serde::Serialize;

use super::OutputFormat;
use crate::models::LocalConfig;

/// Response from login command
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub email: String,
    pub api_key: String,
    pub user_id: String,
}

impl std::fmt::Display for LoginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Logged in as {}", self.email)?;
        writeln!(f, "Your API key: {}", self.api_key)?;
        writeln!(f, "User ID: {}", self.user_id)
    }
}

/// Run the login command
pub async fn run_login(server_url: &str, format: OutputFormat) -> Result<()> {
    // First, check if Google OAuth credentials are configured
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .context("GOOGLE_CLIENT_ID environment variable not set")?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .context("GOOGLE_CLIENT_SECRET environment variable not set")?;

    println!("Starting Google OAuth flow...");

    // Perform OAuth flow
    let creds = crate::calendar::GoogleCalendar::oauth_flow(&client_id, &client_secret)
        .await
        .context("OAuth flow failed")?;

    // Get user email
    let access_token = creds.access_token.as_ref().context("No access token")?;
    let email = crate::calendar::GoogleCalendar::get_user_email(access_token)
        .await
        .context("Failed to get user email")?;

    // Register with the server
    let client = super::get_api_client();
    let resp = client
        .post(format!("{}/auth/register", server_url))
        .json(&serde_json::json!({
            "email": email,
            "refresh_token": creds.refresh_token,
        }))
        .send()
        .await
        .context("Failed to register with server")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Registration failed: {}", error);
    }

    #[derive(serde::Deserialize)]
    struct RegisterResponse {
        user_id: String,
        api_key: String,
    }

    let reg: RegisterResponse = resp.json().await.context("Failed to parse registration response")?;

    // Save config locally
    let mut config = LocalConfig::load().unwrap_or_default();
    config.api_key = Some(reg.api_key.clone());
    config.server_url = Some(server_url.to_string());
    config.email = Some(email.clone());
    config.user_id = Some(reg.user_id.clone());
    config.save().context("Failed to save config")?;

    let response = LoginResponse {
        email,
        api_key: reg.api_key,
        user_id: reg.user_id,
    };

    format.print(&response);
    Ok(())
}

/// Run the logout command
pub fn run_logout(format: OutputFormat) -> Result<()> {
    let config_path = LocalConfig::config_path();
    if config_path.exists() {
        std::fs::remove_file(&config_path).context("Failed to remove config file")?;
    }

    let response = super::SuccessResponse {
        message: "Logged out successfully".to_string(),
    };
    format.print(&response);
    Ok(())
}
