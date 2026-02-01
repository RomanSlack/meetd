mod events;

pub use events::*;

use anyhow::{Context, Result};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Webhook delivery client
pub struct WebhookClient {
    http_client: reqwest::Client,
}

impl WebhookClient {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    /// Deliver a webhook event to the specified URL
    pub async fn deliver(&self, url: &str, secret: &str, event: &WebhookEvent) -> Result<()> {
        let payload = serde_json::to_string(event).context("Failed to serialize webhook event")?;
        let timestamp = Utc::now().timestamp().to_string();

        // Create HMAC signature
        let signature = self.sign(&payload, &timestamp, secret)?;

        let response = self
            .http_client
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-Meetd-Signature", signature)
            .header("X-Meetd-Timestamp", &timestamp)
            .body(payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .context("Failed to send webhook")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Webhook delivery failed: {} - {}", status, body);
        }

        Ok(())
    }

    /// Sign a payload with HMAC-SHA256
    fn sign(&self, payload: &str, timestamp: &str, secret: &str) -> Result<String> {
        let message = format!("{}.{}", timestamp, payload);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).context("Invalid HMAC key")?;
        mac.update(message.as_bytes());
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// Verify an incoming webhook signature
    pub fn verify_signature(
        payload: &str,
        timestamp: &str,
        signature: &str,
        secret: &str,
    ) -> Result<bool> {
        // Check timestamp is not too old (5 minutes)
        let ts: i64 = timestamp.parse().context("Invalid timestamp")?;
        let now = Utc::now().timestamp();
        if (now - ts).abs() > 300 {
            return Ok(false);
        }

        let message = format!("{}.{}", timestamp, payload);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).context("Invalid HMAC key")?;
        mac.update(message.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        Ok(signature == expected)
    }
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_verification() {
        let client = WebhookClient::new();
        let payload = r#"{"event":"test"}"#;
        let timestamp = Utc::now().timestamp().to_string();
        let secret = "test_secret_123";

        let signature = client.sign(payload, &timestamp, secret).unwrap();

        assert!(WebhookClient::verify_signature(payload, &timestamp, &signature, secret).unwrap());

        // Wrong secret should fail
        assert!(
            !WebhookClient::verify_signature(payload, &timestamp, &signature, "wrong_secret")
                .unwrap()
        );
    }
}
