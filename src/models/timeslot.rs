use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A time slot representing a period of time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSlot {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeSlot {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn duration_minutes(&self) -> i64 {
        (self.end - self.start).num_minutes()
    }

    /// Check if this slot overlaps with another
    pub fn overlaps(&self, other: &TimeSlot) -> bool {
        self.start < other.end && self.end > other.start
    }

    /// Check if this slot contains another
    pub fn contains(&self, other: &TimeSlot) -> bool {
        self.start <= other.start && self.end >= other.end
    }
}

/// An available time slot with a score indicating preference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableSlot {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Score from 0.0 to 1.0 indicating preference (higher is better)
    pub score: f64,
}

/// A busy period from the calendar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusyPeriod {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Optional title (depending on visibility settings)
    pub title: Option<String>,
}

/// Request for availability query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityRequest {
    pub with_email: String,
    pub duration_minutes: i32,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    #[serde(default)]
    pub timezone: Option<String>,
}

/// Response for availability query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityResponse {
    pub slots: Vec<AvailableSlot>,
}

/// Time window for queries
#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeWindow {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// Parse a window string like "2026-02-01..2026-02-07"
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.split("..").collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid window format. Use: YYYY-MM-DD..YYYY-MM-DD");
        }

        let start = chrono::NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")?
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let end = chrono::NaiveDate::parse_from_str(parts[1], "%Y-%m-%d")?
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc();

        Ok(Self { start, end })
    }
}
