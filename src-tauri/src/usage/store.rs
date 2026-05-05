// TTP - Talk To Paste
// Persisted usage cache (~/.config/ttp/usage.json)

use crate::licensing::TRIAL_DAYS;
use chrono::{Datelike, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::PathBuf;

/// HMAC secret used to detect tampering of the usage cache.
/// Different from the license secret so each file is independently signed.
const HMAC_SECRET: &[u8; 32] = b"TTPUsageTracking_v1_OneShotTrial";

type HmacSha256 = Hmac<Sha256>;

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
    /// How many times the trial has been started. Once >0, no fresh trial
    /// is granted even if `trial_started_at` is reset by editing the file.
    #[serde(default)]
    pub trial_count: u32,
    /// HMAC of the other fields. Mismatch on load = file was edited.
    #[serde(default)]
    pub signature: Option<String>,
}

fn usage_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("usage.json"))
}

fn compute_usage_signature(record: &UsageRecord) -> String {
    let input = format!(
        "{}|{}|{}|{}",
        record.polish_month,
        record.polish_count,
        record.trial_started_at.unwrap_or(0),
        record.trial_count,
    );
    let mut mac =
        HmacSha256::new_from_slice(HMAC_SECRET).expect("HMAC_SECRET has correct length");
    mac.update(input.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn verify_usage_signature(record: &UsageRecord) -> bool {
    let Some(ref stored) = record.signature else {
        return false;
    };
    compute_usage_signature(record) == *stored
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

    let Ok(content) = fs::read_to_string(&path) else {
        return UsageRecord::default();
    };
    let Ok(mut record) = serde_json::from_str::<UsageRecord>(&content) else {
        return UsageRecord::default();
    };

    if record.signature.is_some() {
        if verify_usage_signature(&record) {
            return record;
        }
        // Tamper detected — discard. User effectively gets a fresh-state
        // record (trial may not restart because trial_count was set on the
        // previous valid save we discarded — but if they delete the file
        // entirely we get UsageRecord::default with trial_count=0 anyway,
        // which is the trade-off of a non-secure store).
        crate::logging::log_warn("usage signature invalid — discarding cache");
        return UsageRecord::default();
    }

    // Legacy 1.6.x file with no signature. If the trial was already started,
    // bump trial_count to 1 so resigning doesn't accidentally hand them a
    // fresh trial via "delete file then relaunch."
    if record.trial_started_at.is_some() && record.trial_count == 0 {
        record.trial_count = 1;
    }
    let _ = save_usage(&record);
    record
}

pub fn save_usage(record: &UsageRecord) -> Result<(), String> {
    let path = usage_path().ok_or("Could not determine config directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let mut record = record.clone();
    record.signature = Some(compute_usage_signature(&record));

    let json = serde_json::to_string_pretty(&record)
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

/// Start the trial if it has not been claimed yet. Returns true on first claim.
/// `trial_count > 0` blocks restart, even if the user has deleted/edited the
/// file (because a new file would be re-signed with trial_count=0, and our
/// signature check prevents text-edited resets).
pub fn start_trial_if_needed() -> bool {
    let mut record = load_usage();
    if record.trial_count > 0 {
        return false;
    }
    record.trial_started_at = Some(Utc::now().timestamp());
    record.trial_count = 1;
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
