use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Only share busy/free status, no details
    #[default]
    BusyOnly,
    /// Share "Busy: Meeting" without title/attendees
    Masked,
    /// Share event titles (never attendees/description)
    Full,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::BusyOnly => "busy_only",
            Visibility::Masked => "masked",
            Visibility::Full => "full",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "busy_only" => Some(Visibility::BusyOnly),
            "masked" => Some(Visibility::Masked),
            "full" => Some(Visibility::Full),
            _ => None,
        }
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub google_refresh_token: Option<String>,
    pub public_key: String,
    #[serde(skip_serializing)]
    pub private_key: String,
    #[serde(skip_serializing)]
    pub api_key_hash: String,
    pub visibility: Visibility,
    pub webhook_url: Option<String>,
    #[serde(skip_serializing)]
    pub webhook_secret: Option<String>,
    pub created_at: i64,
}

/// User info returned by API (excludes sensitive fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub public_key: String,
    pub visibility: Visibility,
    pub webhook_url: Option<String>,
    pub created_at: i64,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            public_key: user.public_key,
            visibility: user.visibility,
            webhook_url: user.webhook_url,
            created_at: user.created_at,
        }
    }
}

/// Configuration stored locally on the CLI
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalConfig {
    pub api_key: Option<String>,
    pub server_url: Option<String>,
    pub email: Option<String>,
    pub user_id: Option<String>,
}

impl LocalConfig {
    pub fn config_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("meetd")
            .join("config.json")
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
