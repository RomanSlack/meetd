use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{CalendarProvider, CreatedEvent};
use crate::models::BusyPeriod;

/// Google Calendar implementation using direct API calls
pub struct GoogleCalendar {
    client: reqwest::Client,
    token_manager: Arc<RwLock<TokenManager>>,
}

struct TokenManager {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    access_token: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

impl TokenManager {
    fn new(client_id: String, client_secret: String, refresh_token: String) -> Self {
        Self {
            client_id,
            client_secret,
            refresh_token,
            access_token: None,
            expires_at: None,
        }
    }

    async fn get_access_token(&mut self, client: &reqwest::Client) -> Result<String> {
        // Check if we have a valid token
        if let (Some(token), Some(expires)) = (&self.access_token, self.expires_at) {
            if Utc::now() < expires - chrono::Duration::minutes(5) {
                return Ok(token.clone());
            }
        }

        // Refresh the token
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("refresh_token", self.refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .context("Failed to refresh token")?;

        if !resp.status().is_success() {
            let error = resp.text().await.unwrap_or_default();
            anyhow::bail!("Token refresh failed: {}", error);
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }

        let tokens: TokenResponse = resp
            .json()
            .await
            .context("Failed to parse token response")?;

        self.access_token = Some(tokens.access_token.clone());
        self.expires_at = Some(Utc::now() + chrono::Duration::seconds(tokens.expires_in));

        Ok(tokens.access_token)
    }
}

impl GoogleCalendar {
    /// Create a new Google Calendar client with the given refresh token
    pub async fn new(client_id: &str, client_secret: &str, refresh_token: &str) -> Result<Self> {
        let client = reqwest::Client::new();
        let token_manager = Arc::new(RwLock::new(TokenManager::new(
            client_id.to_string(),
            client_secret.to_string(),
            refresh_token.to_string(),
        )));

        Ok(Self {
            client,
            token_manager,
        })
    }

    async fn get_token(&self) -> Result<String> {
        let mut manager = self.token_manager.write().await;
        manager.get_access_token(&self.client).await
    }

    /// Perform the OAuth flow and return credentials
    pub async fn oauth_flow(client_id: &str, client_secret: &str) -> Result<OAuthCredentials> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpListener;

        let redirect_uri = "http://localhost:8085";
        let scopes = [
            "https://www.googleapis.com/auth/calendar.readonly",
            "https://www.googleapis.com/auth/calendar.events",
            "https://www.googleapis.com/auth/userinfo.email",
        ]
        .join(" ");

        // Build auth URL
        let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?\
             client_id={}&\
             redirect_uri={}&\
             response_type=code&\
             scope={}&\
             access_type=offline&\
             prompt=consent",
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&scopes)
        );

        // Start local server to receive callback
        let listener = TcpListener::bind("127.0.0.1:8085")
            .await
            .context("Failed to bind to port 8085")?;

        // Open browser
        println!("Opening browser for Google sign-in...");
        if open::that(&auth_url).is_err() {
            println!("Please open this URL in your browser:\n{}", auth_url);
        }

        // Wait for callback
        let (mut socket, _) = listener
            .accept()
            .await
            .context("Failed to accept connection")?;
        let mut reader = BufReader::new(&mut socket);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await?;

        // Parse the code from the request
        let code = request_line
            .split_whitespace()
            .nth(1)
            .and_then(|path| {
                path.split('?').nth(1).and_then(|query| {
                    query.split('&').find_map(|param| {
                        let mut parts = param.split('=');
                        if parts.next() == Some("code") {
                            parts.next().map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                })
            })
            .context("No authorization code in callback")?;

        // Send response to browser
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body><h1>Success!</h1><p>You can close this window.</p></body></html>";
        socket.write_all(response.as_bytes()).await?;

        // Exchange code for tokens
        let client = reqwest::Client::new();
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code.as_str()),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .context("Failed to exchange code for token")?;

        if !resp.status().is_success() {
            let error = resp.text().await.unwrap_or_default();
            anyhow::bail!("Token exchange failed: {}", error);
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
        }

        let tokens: TokenResponse = resp
            .json()
            .await
            .context("Failed to parse token response")?;

        Ok(OAuthCredentials {
            access_token: Some(tokens.access_token),
            refresh_token: tokens
                .refresh_token
                .context("No refresh token in response")?,
        })
    }

    /// Get the user's email from Google
    pub async fn get_user_email(access_token: &str) -> Result<String> {
        let client = reqwest::Client::new();
        let resp = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to fetch user info")?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to get user info: {}", resp.status());
        }

        #[derive(Deserialize)]
        struct UserInfo {
            email: String,
        }

        let info: UserInfo = resp.json().await.context("Failed to parse user info")?;
        Ok(info.email)
    }
}

#[async_trait::async_trait]
impl CalendarProvider for GoogleCalendar {
    async fn get_busy_periods(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<BusyPeriod>> {
        let token = self.get_token().await?;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct FreeBusyRequest {
            time_min: DateTime<Utc>,
            time_max: DateTime<Utc>,
            items: Vec<FreeBusyItem>,
        }

        #[derive(Serialize)]
        struct FreeBusyItem {
            id: String,
        }

        let request = FreeBusyRequest {
            time_min: start,
            time_max: end,
            items: vec![FreeBusyItem {
                id: "primary".to_string(),
            }],
        };

        let resp = self
            .client
            .post("https://www.googleapis.com/calendar/v3/freeBusy")
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .context("Failed to query free/busy")?;

        if !resp.status().is_success() {
            let error = resp.text().await.unwrap_or_default();
            anyhow::bail!("Free/busy query failed: {}", error);
        }

        #[derive(Deserialize)]
        struct FreeBusyResponse {
            calendars: Option<std::collections::HashMap<String, CalendarBusy>>,
        }

        #[derive(Deserialize)]
        struct CalendarBusy {
            busy: Option<Vec<BusySlot>>,
        }

        #[derive(Deserialize)]
        struct BusySlot {
            start: DateTime<Utc>,
            end: DateTime<Utc>,
        }

        let response: FreeBusyResponse = resp
            .json()
            .await
            .context("Failed to parse free/busy response")?;

        let mut busy_periods = Vec::new();

        if let Some(calendars) = response.calendars {
            for (_cal_id, cal_data) in calendars {
                if let Some(busy_list) = cal_data.busy {
                    for busy in busy_list {
                        busy_periods.push(BusyPeriod {
                            start: busy.start,
                            end: busy.end,
                            title: None,
                        });
                    }
                }
            }
        }

        Ok(busy_periods)
    }

    async fn create_event(
        &self,
        title: &str,
        description: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        attendee_email: Option<&str>,
    ) -> Result<CreatedEvent> {
        let token = self.get_token().await?;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct EventRequest {
            summary: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<String>,
            start: EventDateTime,
            end: EventDateTime,
            #[serde(skip_serializing_if = "Option::is_none")]
            attendees: Option<Vec<Attendee>>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct EventDateTime {
            date_time: DateTime<Utc>,
        }

        #[derive(Serialize)]
        struct Attendee {
            email: String,
        }

        let event = EventRequest {
            summary: title.to_string(),
            description: description.map(|s| s.to_string()),
            start: EventDateTime { date_time: start },
            end: EventDateTime { date_time: end },
            attendees: attendee_email.map(|e| {
                vec![Attendee {
                    email: e.to_string(),
                }]
            }),
        };

        let resp = self
            .client
            .post("https://www.googleapis.com/calendar/v3/calendars/primary/events?sendNotifications=true")
            .bearer_auth(&token)
            .json(&event)
            .send()
            .await
            .context("Failed to create event")?;

        if !resp.status().is_success() {
            let error = resp.text().await.unwrap_or_default();
            anyhow::bail!("Event creation failed: {}", error);
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct EventResponse {
            id: Option<String>,
            html_link: Option<String>,
        }

        let created: EventResponse = resp
            .json()
            .await
            .context("Failed to parse event response")?;

        Ok(CreatedEvent {
            id: created.id.unwrap_or_default(),
            html_link: created.html_link,
        })
    }
}

/// OAuth credentials returned from the flow
#[derive(Debug, Clone)]
pub struct OAuthCredentials {
    pub access_token: Option<String>,
    pub refresh_token: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::{find_available_slots, score_slot};

    #[test]
    fn test_find_available_slots() {
        let now = Utc::now();
        let window_start = now;
        let window_end = now + chrono::Duration::hours(8);

        let busy = vec![BusyPeriod {
            start: now + chrono::Duration::hours(2),
            end: now + chrono::Duration::hours(4),
            title: None,
        }];

        let slots = find_available_slots(&busy, window_start, window_end, 30);

        assert!(!slots.is_empty());
        assert_eq!(slots[0].start, window_start);

        for slot in &slots {
            assert!(slot.end <= busy[0].start || slot.start >= busy[0].end);
        }
    }

    #[test]
    fn test_score_slot() {
        let now = Utc::now();

        let good_slot = crate::models::TimeSlot::new(
            now + chrono::Duration::hours(24),
            now + chrono::Duration::hours(24) + chrono::Duration::minutes(30),
        );

        let score = score_slot(&good_slot, now);
        assert!(score > 0.5);
    }
}
