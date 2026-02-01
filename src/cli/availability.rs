use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{get_api_client, get_api_key, get_server_url, OutputFormat};
use crate::models::{AvailabilityRequest, AvailableSlot, TimeWindow};

/// Response from availability query
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct AvailResponse {
    pub slots: Vec<AvailableSlot>,
}

impl std::fmt::Display for AvailResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.slots.is_empty() {
            writeln!(f, "No available slots found in the specified window.")?;
        } else {
            writeln!(f, "Available slots:")?;
            for slot in &self.slots {
                writeln!(
                    f,
                    "  {} - {} (score: {:.2})",
                    slot.start.format("%Y-%m-%d %H:%M"),
                    slot.end.format("%H:%M"),
                    slot.score
                )?;
            }
        }
        Ok(())
    }
}

/// Parse duration string like "30m", "1h", "90m"
pub fn parse_duration(s: &str) -> Result<i32> {
    let s = s.trim().to_lowercase();
    if let Some(mins) = s.strip_suffix('m') {
        mins.parse::<i32>().context("Invalid minutes value")
    } else if let Some(hours) = s.strip_suffix('h') {
        let h: i32 = hours.parse().context("Invalid hours value")?;
        Ok(h * 60)
    } else {
        // Assume minutes if no suffix
        s.parse::<i32>().context("Invalid duration value")
    }
}

/// Query availability for a meeting
pub async fn run_avail(
    with_email: &str,
    duration: &str,
    window: &str,
    timezone: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let api_key = get_api_key()?;
    let server_url = get_server_url();

    let duration_minutes = parse_duration(duration)?;
    let time_window = TimeWindow::parse(window)?;

    let request = AvailabilityRequest {
        with_email: with_email.to_string(),
        duration_minutes,
        window_start: time_window.start,
        window_end: time_window.end,
        timezone: timezone.map(|s| s.to_string()),
    };

    let client = get_api_client();
    let resp = client
        .post(format!("{}/v1/availability", server_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request)
        .send()
        .await
        .context("Failed to query availability")?;

    if !resp.status().is_success() {
        let error = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to query availability: {}", error);
    }

    let response: AvailResponse = resp.json().await.context("Failed to parse response")?;
    format.print(&response);
    Ok(())
}

/// Parse a slot time like "2026-02-03T10:00" or "2026-02-03T10:00:00Z"
pub fn parse_slot_time(s: &str) -> Result<DateTime<Utc>> {
    // Try full RFC3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try without timezone (assume UTC)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.and_utc());
    }

    // Try without seconds
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Ok(dt.and_utc());
    }

    anyhow::bail!("Invalid time format. Use: YYYY-MM-DDTHH:MM or RFC3339")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30m").unwrap(), 30);
        assert_eq!(parse_duration("1h").unwrap(), 60);
        assert_eq!(parse_duration("90m").unwrap(), 90);
        assert_eq!(parse_duration("2h").unwrap(), 120);
        assert_eq!(parse_duration("45").unwrap(), 45);
    }

    #[test]
    fn test_parse_slot_time() {
        let dt = parse_slot_time("2026-02-03T10:00").unwrap();
        assert_eq!(dt.format("%Y-%m-%d %H:%M").to_string(), "2026-02-03 10:00");

        let dt2 = parse_slot_time("2026-02-03T10:00:00Z").unwrap();
        assert_eq!(dt2.format("%Y-%m-%d %H:%M").to_string(), "2026-02-03 10:00");
    }
}
