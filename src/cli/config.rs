use anyhow::{Context, Result};
use serde::Serialize;

use super::{get_api_client, get_api_key, get_server_url, OutputFormat};
use crate::models::{LocalConfig, Visibility};

/// Config show response
#[derive(Debug, Serialize)]
pub struct ConfigShowResponse {
    pub email: Option<String>,
    pub server_url: Option<String>,
    pub visibility: String,
    pub webhook_url: Option<String>,
}

impl std::fmt::Display for ConfigShowResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Email: {}", self.email.as_deref().unwrap_or("not set"))?;
        writeln!(
            f,
            "Server: {}",
            self.server_url.as_deref().unwrap_or("not set")
        )?;
        writeln!(f, "Visibility: {}", self.visibility)?;
        writeln!(
            f,
            "Webhook URL: {}",
            self.webhook_url.as_deref().unwrap_or("not set")
        )
    }
}

/// Show current configuration
pub async fn run_config_show(format: OutputFormat) -> Result<()> {
    let local_config = LocalConfig::load()?;
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .get(format!("{}/v1/config", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .context("Failed to fetch config")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to get config: {}", error);
    }

    #[derive(serde::Deserialize)]
    struct ServerConfig {
        visibility: String,
        webhook_url: Option<String>,
    }

    let server_config: ServerConfig = resp.json().await?;

    let response = ConfigShowResponse {
        email: local_config.email,
        server_url: local_config.server_url,
        visibility: server_config.visibility,
        webhook_url: server_config.webhook_url,
    };

    format.print(&response);
    Ok(())
}

/// Set visibility level
pub async fn run_config_visibility(visibility: &str, format: OutputFormat) -> Result<()> {
    let vis = Visibility::parse(visibility)
        .ok_or_else(|| anyhow::anyhow!("Invalid visibility. Use: busy_only, masked, or full"))?;

    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .patch(format!("{}/v1/config", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "visibility": vis.as_str(),
        }))
        .send()
        .await
        .context("Failed to update config")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to update visibility: {}", error);
    }

    let response = super::SuccessResponse {
        message: format!("Visibility set to: {}", vis),
    };
    format.print(&response);
    Ok(())
}

/// Set webhook URL
pub async fn run_config_webhook(url: Option<&str>, format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .patch(format!("{}/v1/config", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "webhook_url": url,
        }))
        .send()
        .await
        .context("Failed to update config")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to update webhook: {}", error);
    }

    let message = match url {
        Some(u) => format!("Webhook URL set to: {}", u),
        None => "Webhook URL removed".to_string(),
    };

    let response = super::SuccessResponse { message };
    format.print(&response);
    Ok(())
}

/// Set server URL (local only)
pub fn run_config_server(url: &str, format: OutputFormat) -> Result<()> {
    let mut config = LocalConfig::load().unwrap_or_default();
    config.server_url = Some(url.to_string());
    config.save()?;

    let response = super::SuccessResponse {
        message: format!("Server URL set to: {}", url),
    };
    format.print(&response);
    Ok(())
}
