use anyhow::{Context, Result};

use super::{get_api_client, get_api_key, get_server_url, OutputFormat};
use crate::models::AcceptProposalResponse;

impl std::fmt::Display for AcceptProposalResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Status: {}", self.status)?;
        if let Some(event) = &self.event {
            writeln!(f, "Event: {}", event.title)?;
            writeln!(f, "Time: {}", event.start.format("%Y-%m-%d %H:%M %Z"))?;
            if let Some(link) = &event.calendar_link {
                writeln!(f, "Calendar: {}", link)?;
            }
        }
        Ok(())
    }
}

/// Accept a proposal by ID
pub async fn run_accept(proposal_id: &str, format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .post(format!(
            "{}/v1/proposals/{}/accept",
            server_url, proposal_id
        ))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .context("Failed to accept proposal")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to accept proposal: {}", error);
    }

    let response: AcceptProposalResponse = resp.json().await.context("Failed to parse response")?;
    format.print_json(&response);
    Ok(())
}

/// Decline a proposal by ID
pub async fn run_decline(proposal_id: &str, format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .post(format!(
            "{}/v1/proposals/{}/decline",
            server_url, proposal_id
        ))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .context("Failed to decline proposal")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to decline proposal: {}", error);
    }

    let response = super::SuccessResponse {
        message: format!("Proposal {} declined", proposal_id),
    };
    format.print(&response);
    Ok(())
}

/// Accept a proposal via signed payload (agent-to-agent)
pub async fn run_accept_signed(signed_proposal: &str, format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .post(format!("{}/v1/agent/inbox", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "signed_proposal": signed_proposal,
            "action": "accept",
        }))
        .send()
        .await
        .context("Failed to process signed proposal")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to process signed proposal: {}", error);
    }

    let response: AcceptProposalResponse = resp.json().await.context("Failed to parse response")?;
    format.print_json(&response);
    Ok(())
}
