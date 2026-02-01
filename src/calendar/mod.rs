mod google;

pub use google::*;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Timelike, Utc};

use crate::models::{BusyPeriod, TimeSlot};

/// Trait for calendar providers
#[async_trait]
pub trait CalendarProvider: Send + Sync {
    /// Get free/busy information for a time range
    async fn get_busy_periods(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<BusyPeriod>>;

    /// Create a calendar event
    async fn create_event(
        &self,
        title: &str,
        description: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        attendee_email: Option<&str>,
    ) -> Result<CreatedEvent>;
}

/// Result of creating a calendar event
#[derive(Debug, Clone)]
pub struct CreatedEvent {
    pub id: String,
    pub html_link: Option<String>,
}

/// Find available slots given busy periods
pub fn find_available_slots(
    busy_periods: &[BusyPeriod],
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    duration_minutes: i32,
) -> Vec<TimeSlot> {
    let mut available = Vec::new();
    let duration = chrono::Duration::minutes(duration_minutes as i64);

    // Sort busy periods by start time
    let mut sorted_busy: Vec<_> = busy_periods.iter().collect();
    sorted_busy.sort_by_key(|p| p.start);

    let mut current = window_start;

    for busy in sorted_busy {
        // If there's a gap before this busy period, check if it fits the duration
        if busy.start > current {
            let gap_end = busy.start;
            // Add slots in 30-minute increments within the gap
            let mut slot_start = current;
            while slot_start + duration <= gap_end {
                available.push(TimeSlot::new(slot_start, slot_start + duration));
                slot_start += chrono::Duration::minutes(30);
            }
        }
        // Move current to the end of this busy period
        if busy.end > current {
            current = busy.end;
        }
    }

    // Check for available time after the last busy period
    let mut slot_start = current;
    while slot_start + duration <= window_end {
        available.push(TimeSlot::new(slot_start, slot_start + duration));
        slot_start += chrono::Duration::minutes(30);
    }

    available
}

/// Score a time slot based on various factors
pub fn score_slot(slot: &TimeSlot, now: DateTime<Utc>) -> f64 {
    let mut score: f64 = 0.5;

    // Prefer slots during working hours (9 AM - 5 PM UTC-like)
    let hour = slot.start.time().hour();
    if (9..17).contains(&hour) {
        score += 0.2;
    } else if (8..18).contains(&hour) {
        score += 0.1;
    }

    // Prefer weekdays
    let weekday = slot.start.weekday();
    if weekday.num_days_from_monday() < 5 {
        score += 0.1;
    }

    // Prefer slots that are not too soon (give time to prepare)
    let hours_until = (slot.start - now).num_hours();
    if (24..=72).contains(&hours_until) {
        score += 0.1;
    } else if hours_until >= 4 {
        score += 0.05;
    }

    // Prefer slots that start at common meeting times (:00 or :30)
    let minute = slot.start.time().minute();
    if minute == 0 || minute == 30 {
        score += 0.1;
    }

    score.min(1.0_f64)
}

/// Intersect availability between two sets of busy periods
pub fn intersect_availability(
    busy_a: &[BusyPeriod],
    busy_b: &[BusyPeriod],
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    duration_minutes: i32,
) -> Vec<TimeSlot> {
    // Combine all busy periods
    let mut all_busy: Vec<BusyPeriod> = busy_a.to_vec();
    all_busy.extend(busy_b.iter().cloned());

    find_available_slots(&all_busy, window_start, window_end, duration_minutes)
}
