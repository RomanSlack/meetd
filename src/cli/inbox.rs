use anyhow::{Context, Result};
use serde::Serialize;

use super::{get_api_client, get_api_key, get_server_url, OutputFormat};
use crate::models::InboxProposal;

/// Inbox list response
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct InboxResponse {
    pub proposals: Vec<InboxProposal>,
}

impl std::fmt::Display for InboxResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.proposals.is_empty() {
            writeln!(f, "No pending proposals.")?;
        } else {
            writeln!(f, "Pending proposals:")?;
            for p in &self.proposals {
                writeln!(
                    f,
                    "  {} - {} at {} ({} min)",
                    p.id,
                    p.title.as_deref().unwrap_or("(no title)"),
                    p.slot.start.format("%Y-%m-%d %H:%M"),
                    p.slot.duration_minutes
                )?;
                writeln!(f, "    From: {}", p.from)?;
                writeln!(f, "    Expires: {}", p.expires_at.format("%Y-%m-%d %H:%M"))?;
            }
        }
        Ok(())
    }
}

/// List inbox proposals
pub async fn run_inbox(status: Option<&str>, format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let url = match status {
        Some(s) => format!("{}/v1/inbox?status={}", server_url, s),
        None => format!("{}/v1/inbox", server_url),
    };

    let client = get_api_client();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .context("Failed to fetch inbox")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch inbox: {}", error);
    }

    let response: InboxResponse = resp.json().await.context("Failed to parse response")?;
    format.print(&response);
    Ok(())
}

/// Watch for new proposals (polling mode)
pub async fn run_inbox_watch(format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    println!("Watching for new proposals... (press Ctrl+C to stop)");

    let client = get_api_client();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Get initial proposals
    let resp = client
        .get(format!("{}/v1/inbox?status=pending", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .context("Failed to fetch inbox")?;

    if resp.status().is_success() {
        if let Ok(initial) = resp.json::<InboxResponse>().await {
            for p in initial.proposals {
                seen_ids.insert(p.id);
            }
        }
    }

    // Poll for new proposals
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let resp = client
            .get(format!("{}/v1/inbox?status=pending", server_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await;

        if let Ok(resp) = resp {
            if resp.status().is_success() {
                if let Ok(inbox) = resp.json::<InboxResponse>().await {
                    for proposal in inbox.proposals {
                        if !seen_ids.contains(&proposal.id) {
                            seen_ids.insert(proposal.id.clone());
                            match format {
                                OutputFormat::Json => {
                                    println!("{}", serde_json::to_string(&proposal).unwrap());
                                }
                                OutputFormat::Human => {
                                    println!(
                                        "New proposal: {} from {} at {}",
                                        proposal.title.as_deref().unwrap_or("(no title)"),
                                        proposal.from,
                                        proposal.slot.start.format("%Y-%m-%d %H:%M")
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Get sent proposals
pub async fn run_sent(format: OutputFormat) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let client = get_api_client();
    let resp = client
        .get(format!("{}/v1/proposals/sent", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .context("Failed to fetch sent proposals")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch sent proposals: {}", error);
    }

    let response: InboxResponse = resp.json().await.context("Failed to parse response")?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&response).unwrap());
        }
        OutputFormat::Human => {
            if response.proposals.is_empty() {
                println!("No sent proposals.");
            } else {
                println!("Sent proposals:");
                for p in &response.proposals {
                    println!(
                        "  {} - {} at {} ({} min) [{}]",
                        p.id,
                        p.title.as_deref().unwrap_or("(no title)"),
                        p.slot.start.format("%Y-%m-%d %H:%M"),
                        p.slot.duration_minutes,
                        p.status
                    );
                }
            }
        }
    }

    Ok(())
}
