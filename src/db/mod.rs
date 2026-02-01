use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::models::{Proposal, ProposalStatus, User, Visibility};

const MIGRATION_001: &str = include_str!("migrations/001_initial.sql");

/// Database connection wrapper
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(MIGRATION_001)
            .context("Failed to run migration 001")?;
        Ok(())
    }

    // ==================== User Operations ====================

    /// Create a new user
    pub fn create_user(&self, user: &User) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO users (id, email, google_refresh_token, public_key, private_key,
                              api_key_hash, visibility, webhook_url, webhook_secret, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                user.id,
                user.email,
                user.google_refresh_token,
                user.public_key,
                user.private_key,
                user.api_key_hash,
                user.visibility.as_str(),
                user.webhook_url,
                user.webhook_secret,
                user.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get a user by ID
    pub fn get_user(&self, id: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, google_refresh_token, public_key, private_key,
                    api_key_hash, visibility, webhook_url, webhook_secret, created_at
             FROM users WHERE id = ?1",
        )?;

        stmt.query_row(params![id], |row| {
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                google_refresh_token: row.get(2)?,
                public_key: row.get(3)?,
                private_key: row.get(4)?,
                api_key_hash: row.get(5)?,
                visibility: Visibility::parse(&row.get::<_, String>(6)?).unwrap_or_default(),
                webhook_url: row.get(7)?,
                webhook_secret: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .optional()
        .context("Failed to get user")
    }

    /// Get a user by email
    pub fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, google_refresh_token, public_key, private_key,
                    api_key_hash, visibility, webhook_url, webhook_secret, created_at
             FROM users WHERE email = ?1",
        )?;

        stmt.query_row(params![email], |row| {
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                google_refresh_token: row.get(2)?,
                public_key: row.get(3)?,
                private_key: row.get(4)?,
                api_key_hash: row.get(5)?,
                visibility: Visibility::parse(&row.get::<_, String>(6)?).unwrap_or_default(),
                webhook_url: row.get(7)?,
                webhook_secret: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .optional()
        .context("Failed to get user by email")
    }

    /// Update user's refresh token
    pub fn update_user_refresh_token(&self, user_id: &str, token: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET google_refresh_token = ?1 WHERE id = ?2",
            params![token, user_id],
        )?;
        Ok(())
    }

    /// Update user's visibility setting
    pub fn update_user_visibility(&self, user_id: &str, visibility: Visibility) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET visibility = ?1 WHERE id = ?2",
            params![visibility.as_str(), user_id],
        )?;
        Ok(())
    }

    /// Update user's webhook configuration
    pub fn update_user_webhook(
        &self,
        user_id: &str,
        webhook_url: Option<&str>,
        webhook_secret: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET webhook_url = ?1, webhook_secret = ?2 WHERE id = ?3",
            params![webhook_url, webhook_secret, user_id],
        )?;
        Ok(())
    }

    /// Update user's API key hash
    pub fn update_user_api_key_hash(&self, user_id: &str, api_key_hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET api_key_hash = ?1 WHERE id = ?2",
            params![api_key_hash, user_id],
        )?;
        Ok(())
    }

    /// Find user by validating API key against stored hashes
    /// Returns the user if the API key matches
    pub fn find_user_by_api_key(&self, api_key: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, google_refresh_token, public_key, private_key,
                    api_key_hash, visibility, webhook_url, webhook_secret, created_at
             FROM users",
        )?;

        let users = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                google_refresh_token: row.get(2)?,
                public_key: row.get(3)?,
                private_key: row.get(4)?,
                api_key_hash: row.get(5)?,
                visibility: Visibility::parse(&row.get::<_, String>(6)?).unwrap_or_default(),
                webhook_url: row.get(7)?,
                webhook_secret: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        for user_result in users {
            let user = user_result?;
            if bcrypt::verify(api_key, &user.api_key_hash).unwrap_or(false) {
                return Ok(Some(user));
            }
        }

        Ok(None)
    }

    // ==================== Proposal Operations ====================

    /// Create a new proposal
    pub fn create_proposal(&self, proposal: &Proposal) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO proposals (id, from_user_id, to_email, slot_start, duration_minutes,
                                   title, description, nonce, expires_at, signature, status, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                proposal.id,
                proposal.from_user_id,
                proposal.to_email,
                proposal.slot_start.timestamp(),
                proposal.duration_minutes,
                proposal.title,
                proposal.description,
                proposal.nonce,
                proposal.expires_at.timestamp(),
                proposal.signature,
                proposal.status.as_str(),
                proposal.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get a proposal by ID
    pub fn get_proposal(&self, id: &str) -> Result<Option<Proposal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, from_user_id, to_email, slot_start, duration_minutes, title, description,
                    nonce, expires_at, signature, status, created_at
             FROM proposals WHERE id = ?1",
        )?;

        stmt.query_row(params![id], |row| {
            Ok(Proposal {
                id: row.get(0)?,
                from_user_id: row.get(1)?,
                to_email: row.get(2)?,
                slot_start: DateTime::from_timestamp(row.get(3)?, 0).unwrap_or_else(Utc::now),
                duration_minutes: row.get(4)?,
                title: row.get(5)?,
                description: row.get(6)?,
                nonce: row.get(7)?,
                expires_at: DateTime::from_timestamp(row.get(8)?, 0).unwrap_or_else(Utc::now),
                signature: row.get(9)?,
                status: ProposalStatus::parse(&row.get::<_, String>(10)?).unwrap_or_default(),
                created_at: row.get(11)?,
            })
        })
        .optional()
        .context("Failed to get proposal")
    }

    /// Get proposals for a recipient
    pub fn get_proposals_for_email(
        &self,
        email: &str,
        status: Option<ProposalStatus>,
    ) -> Result<Vec<Proposal>> {
        let conn = self.conn.lock().unwrap();

        let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match status {
            Some(s) => (
                "SELECT id, from_user_id, to_email, slot_start, duration_minutes, title, description,
                        nonce, expires_at, signature, status, created_at
                 FROM proposals WHERE to_email = ?1 AND status = ?2
                 ORDER BY slot_start ASC",
                vec![Box::new(email.to_string()), Box::new(s.as_str().to_string())],
            ),
            None => (
                "SELECT id, from_user_id, to_email, slot_start, duration_minutes, title, description,
                        nonce, expires_at, signature, status, created_at
                 FROM proposals WHERE to_email = ?1
                 ORDER BY slot_start ASC",
                vec![Box::new(email.to_string())],
            ),
        };

        let mut stmt = conn.prepare(sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let proposals = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(Proposal {
                id: row.get(0)?,
                from_user_id: row.get(1)?,
                to_email: row.get(2)?,
                slot_start: DateTime::from_timestamp(row.get(3)?, 0).unwrap_or_else(Utc::now),
                duration_minutes: row.get(4)?,
                title: row.get(5)?,
                description: row.get(6)?,
                nonce: row.get(7)?,
                expires_at: DateTime::from_timestamp(row.get(8)?, 0).unwrap_or_else(Utc::now),
                signature: row.get(9)?,
                status: ProposalStatus::parse(&row.get::<_, String>(10)?).unwrap_or_default(),
                created_at: row.get(11)?,
            })
        })?;

        proposals
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to get proposals")
    }

    /// Get proposals sent by a user
    pub fn get_proposals_from_user(&self, user_id: &str) -> Result<Vec<Proposal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, from_user_id, to_email, slot_start, duration_minutes, title, description,
                    nonce, expires_at, signature, status, created_at
             FROM proposals WHERE from_user_id = ?1
             ORDER BY created_at DESC",
        )?;

        let proposals = stmt.query_map(params![user_id], |row| {
            Ok(Proposal {
                id: row.get(0)?,
                from_user_id: row.get(1)?,
                to_email: row.get(2)?,
                slot_start: DateTime::from_timestamp(row.get(3)?, 0).unwrap_or_else(Utc::now),
                duration_minutes: row.get(4)?,
                title: row.get(5)?,
                description: row.get(6)?,
                nonce: row.get(7)?,
                expires_at: DateTime::from_timestamp(row.get(8)?, 0).unwrap_or_else(Utc::now),
                signature: row.get(9)?,
                status: ProposalStatus::parse(&row.get::<_, String>(10)?).unwrap_or_default(),
                created_at: row.get(11)?,
            })
        })?;

        proposals
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to get proposals from user")
    }

    /// Update proposal status
    pub fn update_proposal_status(&self, id: &str, status: ProposalStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE proposals SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id],
        )?;
        Ok(())
    }

    /// Expire old pending proposals
    pub fn expire_old_proposals(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        let count = conn.execute(
            "UPDATE proposals SET status = 'expired' WHERE status = 'pending' AND expires_at < ?1",
            params![now],
        )?;
        Ok(count)
    }

    // ==================== Nonce Operations ====================

    /// Check if a nonce has been used
    pub fn is_nonce_used(&self, nonce: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT 1 FROM used_nonces WHERE nonce = ?1")?;
        let exists = stmt.exists(params![nonce])?;
        Ok(exists)
    }

    /// Mark a nonce as used
    pub fn use_nonce(&self, nonce: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT OR IGNORE INTO used_nonces (nonce, used_at) VALUES (?1, ?2)",
            params![nonce, now],
        )?;
        Ok(())
    }

    /// Clean up old nonces (older than 24 hours)
    pub fn cleanup_old_nonces(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let cutoff = Utc::now().timestamp() - 86400; // 24 hours ago
        let count = conn.execute(
            "DELETE FROM used_nonces WHERE used_at < ?1",
            params![cutoff],
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Visibility;

    fn create_test_user() -> User {
        User {
            id: uuid::Uuid::new_v4().to_string(),
            email: "test@example.com".to_string(),
            google_refresh_token: Some("token123".to_string()),
            public_key: "pubkey".to_string(),
            private_key: "privkey".to_string(),
            api_key_hash: bcrypt::hash("test_api_key", 4).unwrap(),
            visibility: Visibility::BusyOnly,
            webhook_url: None,
            webhook_secret: None,
            created_at: Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_create_and_get_user() {
        let db = Database::open_in_memory().unwrap();
        let user = create_test_user();

        db.create_user(&user).unwrap();

        let retrieved = db.get_user(&user.id).unwrap().unwrap();
        assert_eq!(retrieved.email, user.email);
        assert_eq!(retrieved.public_key, user.public_key);
    }

    #[test]
    fn test_find_user_by_api_key() {
        let db = Database::open_in_memory().unwrap();
        let user = create_test_user();

        db.create_user(&user).unwrap();

        let found = db.find_user_by_api_key("test_api_key").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, user.id);

        let not_found = db.find_user_by_api_key("wrong_key").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_nonce_tracking() {
        let db = Database::open_in_memory().unwrap();

        assert!(!db.is_nonce_used("test-nonce").unwrap());

        db.use_nonce("test-nonce").unwrap();

        assert!(db.is_nonce_used("test-nonce").unwrap());
    }
}
