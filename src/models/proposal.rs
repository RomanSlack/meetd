use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    #[default]
    Pending,
    Accepted,
    Declined,
    Expired,
}

impl ProposalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalStatus::Pending => "pending",
            ProposalStatus::Accepted => "accepted",
            ProposalStatus::Declined => "declined",
            ProposalStatus::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ProposalStatus::Pending),
            "accepted" => Some(ProposalStatus::Accepted),
            "declined" => Some(ProposalStatus::Declined),
            "expired" => Some(ProposalStatus::Expired),
            _ => None,
        }
    }
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A meeting proposal stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub from_user_id: String,
    pub to_email: String,
    pub slot_start: DateTime<Utc>,
    pub duration_minutes: i32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
    pub signature: String,
    pub status: ProposalStatus,
    pub created_at: i64,
}

/// Signed proposal format for agent-to-agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedProposal {
    pub version: u32,
    pub from: String,
    pub from_pubkey: String,
    pub to: String,
    pub slot: ProposalSlot,
    pub title: Option<String>,
    pub description: Option<String>,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSlot {
    pub start: DateTime<Utc>,
    pub duration_minutes: i32,
}

impl SignedProposal {
    /// Get the canonical string to sign
    pub fn signing_payload(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            self.version,
            self.from,
            self.to,
            self.slot.start.to_rfc3339(),
            self.slot.duration_minutes,
            self.title.as_deref().unwrap_or(""),
            self.nonce,
            self.expires_at.to_rfc3339()
        )
    }
}

/// Request to create a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProposalRequest {
    pub to_email: String,
    pub slot_start: DateTime<Utc>,
    pub duration_minutes: i32,
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Response after creating a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProposalResponse {
    pub proposal_id: String,
    pub signed_proposal: String,
    pub accept_link: String,
}

/// Proposal info for inbox listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxProposal {
    pub id: String,
    pub from: String,
    pub slot: ProposalSlot,
    pub title: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub status: ProposalStatus,
}

impl From<Proposal> for InboxProposal {
    fn from(p: Proposal) -> Self {
        Self {
            id: p.id,
            from: p.from_user_id,
            slot: ProposalSlot {
                start: p.slot_start,
                duration_minutes: p.duration_minutes,
            },
            title: p.title,
            expires_at: p.expires_at,
            status: p.status,
        }
    }
}

/// Response after accepting a proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptProposalResponse {
    pub status: String,
    pub event: Option<CalendarEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub calendar_link: Option<String>,
}
