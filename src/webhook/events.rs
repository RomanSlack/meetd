use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::ProposalSlot;

/// Types of webhook events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// New meeting proposal received
    ProposalReceived,
    /// Your proposal was accepted
    ProposalAccepted,
    /// Your proposal was declined
    ProposalDeclined,
    /// A proposal expired without response
    ProposalExpired,
}

impl WebhookEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            WebhookEventType::ProposalReceived => "proposal.received",
            WebhookEventType::ProposalAccepted => "proposal.accepted",
            WebhookEventType::ProposalDeclined => "proposal.declined",
            WebhookEventType::ProposalExpired => "proposal.expired",
        }
    }
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Webhook event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Event type
    pub event: String,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Event-specific data
    pub data: WebhookEventData,
}

impl WebhookEvent {
    pub fn new(event_type: WebhookEventType, data: WebhookEventData) -> Self {
        Self {
            event: event_type.as_str().to_string(),
            timestamp: Utc::now(),
            data,
        }
    }
}

/// Event-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEventData {
    /// Proposal ID
    pub proposal_id: String,
    /// Sender email
    pub from: String,
    /// Sender's public key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_pubkey: Option<String>,
    /// Proposed time slot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<ProposalSlot>,
    /// Meeting title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// When the proposal expires
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Signature of the original proposal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Calendar event link (for accepted proposals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_link: Option<String>,
}

impl WebhookEventData {
    /// Create data for a proposal.received event
    pub fn proposal_received(
        proposal_id: String,
        from: String,
        from_pubkey: String,
        slot: ProposalSlot,
        title: Option<String>,
        expires_at: DateTime<Utc>,
        signature: String,
    ) -> Self {
        Self {
            proposal_id,
            from,
            from_pubkey: Some(from_pubkey),
            slot: Some(slot),
            title,
            expires_at: Some(expires_at),
            signature: Some(signature),
            calendar_link: None,
        }
    }

    /// Create data for a proposal.accepted event
    pub fn proposal_accepted(
        proposal_id: String,
        from: String,
        calendar_link: Option<String>,
    ) -> Self {
        Self {
            proposal_id,
            from,
            from_pubkey: None,
            slot: None,
            title: None,
            expires_at: None,
            signature: None,
            calendar_link,
        }
    }

    /// Create data for a proposal.declined event
    pub fn proposal_declined(proposal_id: String, from: String) -> Self {
        Self {
            proposal_id,
            from,
            from_pubkey: None,
            slot: None,
            title: None,
            expires_at: None,
            signature: None,
            calendar_link: None,
        }
    }

    /// Create data for a proposal.expired event
    pub fn proposal_expired(proposal_id: String, from: String) -> Self {
        Self {
            proposal_id,
            from,
            from_pubkey: None,
            slot: None,
            title: None,
            expires_at: None,
            signature: None,
            calendar_link: None,
        }
    }
}
