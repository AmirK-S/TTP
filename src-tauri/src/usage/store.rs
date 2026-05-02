// TTP - Talk To Paste
// Persisted usage cache (~/.config/ttp/usage.json)

use crate::licensing::TRIAL_DAYS;
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Persisted usage record.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageRecord {
    /// "YYYY-MM" string for the month the polish counter is currently tracking.
    #[serde(default)]
    pub polish_month: String,
    /// Number of successful polishes in `polish_month`.
    #[serde(default)]
    pub polish_count: u32,
    /// Unix timestamp when the local trial started. None until first launch.
    #[serde(default)]
    pub trial_started_at: Option<i64>,
}

fn usage_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("usage.json"))
}

pub fn current_month_key() -> String {
    let now = Utc::now();
    format!("{:04}-{:02}", now.year(), now.month())
}

pub fn load_usage() -> UsageRecord {
    let Some(path) = usage_path() else {
        return UsageRecord::default();
    };
    if !path.exists() {
        return UsageRecord::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_usage(record: &UsageRecord) -> Result<(), String> {
    let path = usage_path().ok_or("Could not determine config directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| format!("Failed to serialize usage: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write usage file: {}", e))?;
    Ok(())
}

/// Returns the polish count for the current month (auto-resets if month rolled over).
pub fn polish_count_this_month(record: &UsageRecord) -> u32 {
    if record.polish_month == current_month_key() {
        record.polish_count
    } else {
        0
    }
}

/// Increment the polish counter, resetting if month changed. Best-effort persist.
pub fn record_polish_success() {
    let mut record = load_usage();
    let month = current_month_key();
    if record.polish_month != month {
        record.polish_month = month;
        record.polish_count = 0;
    }
    record.polish_count = record.polish_count.saturating_add(1);
    if let Err(e) = save_usage(&record) {
        eprintln!("[Usage] Failed to persist polish count: {}", e);
    }
}

/// Start the trial if it has not started yet. Returns true if the trial was just started.
pub fn start_trial_if_needed() -> bool {
    let mut record = load_usage();
    if record.trial_started_at.is_some() {
        return false;
    }
    record.trial_started_at = Some(Utc::now().timestamp());
    if let Err(e) = save_usage(&record) {
        eprintln!("[Usage] Failed to persist trial start: {}", e);
        return false;
    }
    true
}

pub fn trial_started_at(record: &UsageRecord) -> Option<i64> {
    record.trial_started_at
}

/// Days remaining in the trial (0 if expired). Negative is clamped to 0.
pub fn trial_days_left(record: &UsageRecord) -> i64 {
    let Some(started) = record.trial_started_at else {
        return 0;
    };
    let elapsed_secs = Utc::now().timestamp().saturating_sub(started);
    let elapsed_days = elapsed_secs / 86_400;
    (TRIAL_DAYS - elapsed_days).max(0)
}
