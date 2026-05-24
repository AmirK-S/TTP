// TTP - Talk To Paste
// Persisted usage cache (~/.config/ttp/usage.json)

use crate::licensing::TRIAL_DAYS;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::BTreeMap;
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

/// Per-day usage stats for the in-app analytics panel. One entry per day
/// that had at least one successful transcription. Stored under a "YYYY-MM-DD"
/// key in `UsageRecord.daily_stats`. Bumped once per successful pipeline run
/// from `pipeline.rs`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyStats {
    #[serde(default)]
    pub transcriptions: u32,
    #[serde(default)]
    pub words: u64,
    #[serde(default)]
    pub chars: u64,
}

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
    /// Per-day usage stats keyed by "YYYY-MM-DD". BTreeMap keeps keys ordered
    /// so signature input is deterministic regardless of insertion order.
    #[serde(default)]
    pub daily_stats: BTreeMap<String, DailyStats>,
    /// HMAC of the other fields. Mismatch on load = file was edited.
    #[serde(default)]
    pub signature: Option<String>,
}

fn usage_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("usage.json"))
}

fn usage_signature_input(record: &UsageRecord) -> String {
    let base = format!(
        "{}|{}|{}|{}",
        record.polish_month,
        record.polish_count,
        record.trial_started_at.unwrap_or(0),
        record.trial_count,
    );
    // Backward-compat: pre-2.0.5 records didn't include daily_stats in their
    // signature input. Keep the old format when daily_stats is empty so an
    // upgraded user's existing usage.json verifies on first load, then gets
    // re-signed with the new format the first time they transcribe.
    if record.daily_stats.is_empty() {
        return base;
    }
    // BTreeMap iteration is key-ordered, so the serialized string is
    // deterministic regardless of insertion order.
    let daily = record
        .daily_stats
        .iter()
        .map(|(date, s)| format!("{}:{}:{}:{}", date, s.transcriptions, s.words, s.chars))
        .collect::<Vec<_>>()
        .join(";");
    format!("{}|{}", base, daily)
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

fn current_day_key() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
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

/// Record a successful transcription for today's date bucket. Best-effort:
/// failures to persist are logged but don't disrupt the pipeline (the
/// transcription already completed, the user got their text).
pub fn record_transcription(words: u32, chars: u32) {
    let mut record = load_usage();
    let today = current_day_key();
    let entry = record.daily_stats.entry(today).or_default();
    entry.transcriptions = entry.transcriptions.saturating_add(1);
    entry.words = entry.words.saturating_add(words as u64);
    entry.chars = entry.chars.saturating_add(chars as u64);
    if let Err(e) = save_usage(&record) {
        eprintln!("[Usage] Failed to persist daily transcription stats: {}", e);
    }
}

/// Aggregated stats for a given time window. Counts are summed across all
/// daily buckets that fall inside the window (inclusive of both endpoints).
#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsWindow {
    pub transcriptions: u32,
    pub words: u64,
    pub chars: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsSummary {
    pub week: AnalyticsWindow,
    pub month: AnalyticsWindow,
    pub all_time: AnalyticsWindow,
    /// Last 30 daily buckets ordered by date ascending. Sparse: only days
    /// with at least one transcription are present.
    pub daily: Vec<DailyPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyPoint {
    pub date: String,
    pub transcriptions: u32,
    pub words: u64,
    pub chars: u64,
}

fn empty_window() -> AnalyticsWindow {
    AnalyticsWindow {
        transcriptions: 0,
        words: 0,
        chars: 0,
    }
}

fn add_to_window(window: &mut AnalyticsWindow, stats: &DailyStats) {
    window.transcriptions = window.transcriptions.saturating_add(stats.transcriptions);
    window.words = window.words.saturating_add(stats.words);
    window.chars = window.chars.saturating_add(stats.chars);
}

/// Marker file that records the month key ("YYYY-MM") for which we last
/// surfaced the "AI Polish cap hit" system notification. Unsigned on purpose:
/// this is purely a UX dedupe, not a security-relevant counter. The cap itself
/// is still enforced in pipeline.rs against the signed `polish_count`.
fn polish_cap_notify_marker_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("polish_cap_notified"))
}

/// Returns true the first time it's called within a calendar month, false on
/// every subsequent call until the month rolls over. Used to throttle the
/// upgrade notification so a free user who's at cap doesn't get a fresh toast
/// every single transcription — on Windows the OS doesn't group repeats and
/// the spam was reported as "giga chiant" (and read as bloatware).
///
/// Best-effort: if the config dir isn't resolvable or the file write fails,
/// we still return true so the user sees the message at least that one time.
pub fn should_notify_polish_cap_once_this_month() -> bool {
    let Some(path) = polish_cap_notify_marker_path() else {
        return true;
    };
    let current = current_month_key();
    if let Ok(last) = fs::read_to_string(&path) {
        if last.trim() == current {
            return false;
        }
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, &current);
    true
}

/// Aggregate the daily buckets into rolling-window totals + a 30-day series.
pub fn analytics_summary() -> AnalyticsSummary {
    let record = load_usage();
    let today = Utc::now().date_naive();
    let week_start = today - Duration::days(6);
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let chart_start = today - Duration::days(29);

    let mut week = empty_window();
    let mut month = empty_window();
    let mut all_time = empty_window();
    let mut daily = Vec::new();

    for (date_key, stats) in record.daily_stats.iter() {
        let parsed = match NaiveDate::parse_from_str(date_key, "%Y-%m-%d") {
            Ok(d) => d,
            // Skip malformed entries silently — we never want analytics to
            // panic, and a bad key is unrecoverable but harmless.
            Err(_) => continue,
        };
        add_to_window(&mut all_time, stats);
        if parsed >= month_start && parsed <= today {
            add_to_window(&mut month, stats);
        }
        if parsed >= week_start && parsed <= today {
            add_to_window(&mut week, stats);
        }
        if parsed >= chart_start && parsed <= today {
            daily.push(DailyPoint {
                date: date_key.clone(),
                transcriptions: stats.transcriptions,
                words: stats.words,
                chars: stats.chars,
            });
        }
    }

    AnalyticsSummary {
        week,
        month,
        all_time,
        daily,
    }
}
