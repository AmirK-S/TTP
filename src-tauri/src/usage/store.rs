// TTP - Talk To Paste
// Persisted usage cache (~/.config/ttp/usage.json)

use crate::licensing::TRIAL_DAYS;
use chrono::{Datelike, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::PathBuf;

/// Legacy hardcoded HMAC secret from v1.6.x–v1.7.3. Kept as a fallback so
/// usage.json files signed by previous versions still verify on first run
/// after upgrade — we re-sign them with the per-machine key below.
const LEGACY_HMAC_SECRET: &[u8; 32] = b"TTPUsageTracking_v1_OneShotTrial";

const KEYCHAIN_ACCOUNT: &str = "usage_hmac_secret";

fn machine_hmac_secret() -> [u8; 32] {
    crate::keychain::get_or_create_hmac_secret(KEYCHAIN_ACCOUNT, LEGACY_HMAC_SECRET)
}

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

fn usage_signature_input(record: &UsageRecord) -> String {
    format!(
        "{}|{}|{}|{}",
        record.polish_month,
        record.polish_count,
        record.trial_started_at.unwrap_or(0),
        record.trial_count,
    )
}

fn compute_usage_signature(record: &UsageRecord) -> String {
    let input = usage_signature_input(record);
    let secret = machine_hmac_secret();
    let mut mac = HmacSha256::new_from_slice(&secret).expect("32-byte secret");
    mac.update(input.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

enum SigVerify {
    NotPresent,
    Invalid,
    ValidMachine,
    ValidLegacy,
}

/// Constant-time HMAC verification via `verify_slice`. Tries the per-machine
/// secret first, falls back to the legacy hardcoded constant so usage.json
/// files written by previous versions still verify on first run after upgrade.
fn verify_usage_signature(record: &UsageRecord) -> SigVerify {
    let Some(stored_hex) = record.signature.as_deref() else {
        return SigVerify::NotPresent;
    };
    let Ok(stored_bytes) = hex::decode(stored_hex) else {
        return SigVerify::Invalid;
    };

    let input = usage_signature_input(record);

    let machine_secret = machine_hmac_secret();
    let mut mac = HmacSha256::new_from_slice(&machine_secret).expect("32-byte secret");
    mac.update(input.as_bytes());
    if mac.verify_slice(&stored_bytes).is_ok() {
        return SigVerify::ValidMachine;
    }

    let mut legacy = HmacSha256::new_from_slice(LEGACY_HMAC_SECRET).expect("32-byte secret");
    legacy.update(input.as_bytes());
    if legacy.verify_slice(&stored_bytes).is_ok() {
        return SigVerify::ValidLegacy;
    }
    SigVerify::Invalid
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

    match verify_usage_signature(&record) {
        SigVerify::ValidMachine => record,
        SigVerify::ValidLegacy => {
            // Re-sign with the per-machine secret so the legacy path stops
            // being needed on this install.
            let _ = save_usage(&record);
            record
        }
        SigVerify::NotPresent => {
            // Pre-1.6.2 file with no signature. If the trial was already
            // started, bump trial_count to 1 so resigning doesn't accidentally
            // hand them a fresh trial via "delete file then relaunch."
            if record.trial_started_at.is_some() && record.trial_count == 0 {
                record.trial_count = 1;
            }
            let _ = save_usage(&record);
            record
        }
        SigVerify::Invalid => {
            // Tamper detected — discard.
            crate::logging::log_warn("usage signature invalid — discarding cache");
            UsageRecord::default()
        }
    }
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
