use anyhow::{Context, Result};
use serde::Serialize;

use super::{get_api_client, get_api_key, get_server_url, OutputFormat};
use crate::cli::availability::parse_slot_time;
use crate::models::CreateProposalRequest;

/// Response from creating a proposal
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ProposeResponse {
    pub proposal_id: String,
    pub signed_proposal: String,
    pub accept_link: String,
}

impl std::fmt::Display for ProposeResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Proposal created!")?;
        writeln!(f, "Proposal ID: {}", self.proposal_id)?;
        writeln!(f, "Accept link: {}", self.accept_link)?;
        writeln!(f)?;
        writeln!(f, "Signed proposal (for agent-to-agent):")?;
        writeln!(f, "{}", self.signed_proposal)
    }
}

/// Create a meeting proposal
pub async fn run_propose(
    to_email: &str,
    slot: &str,
    duration: &str,
    title: Option<&str>,
    description: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let slot_start = parse_slot_time(slot)?;
    let duration_minutes = super::availability::parse_duration(duration)?;

    let request = CreateProposalRequest {
        to_email: to_email.to_string(),
        slot_start,
        duration_minutes,
        title: title.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
    };

    let client = get_api_client();
    let resp = client
        .post(format!("{}/v1/proposals", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request)
        .send()
        .await
        .context("Failed to create proposal")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to create proposal: {}", error);
    }

    let response: ProposeResponse = resp.json().await.context("Failed to parse response")?;
    format.print(&response);
    Ok(())
}
