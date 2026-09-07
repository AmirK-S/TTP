// TTP - Talk To Paste
// Usage tracking - polish counter (monthly), trial state

mod store;

pub use store::{
    AnalyticsSummary, UsageRecord, analytics_summary, load_usage,
    polish_count_this_month, record_polish_success, record_transcription,
    start_trial_if_needed, trial_days_left, warm_keychain_cache,
};

use serde::Serialize;

use crate::licensing;
use crate::licensing::TRIAL_DAYS;

/// Snapshot of usage state, exposed to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct UsageStats {
    pub is_pro: bool,
    pub is_in_trial: bool,
    pub trial_days_left: Option<i64>,
    pub trial_started_at: Option<i64>,
    /// Exact UTC second at which the trial ends. Single source of truth for
    /// the onboarding countdown chip — without this, the JS side would have
    /// to know `TRIAL_DAYS` and drift if the backend ever changes it (it has
    /// been 7→3→4 historically, and v2.x onboarding shipped a 7-day countdown
    /// against a 4-day backend, lying to brand-new users for two minor versions).
    pub trial_ends_at: Option<i64>,
    // Counts only. The `*_limit_free` companions are gone along with the
    // caps themselves — nothing is capped, so there is no limit to report and
    // no "23 / 30" for the UI to render as a countdown to a paywall.
    pub polish_count_this_month: u32,
    pub dictionary_count: usize,
    pub history_count: usize,
}

#[tauri::command]
pub fn get_usage_stats() -> UsageStats {
    let usage = load_usage();
    let is_pro = licensing::is_pro_disk();
    let is_in_trial = licensing::is_in_trial_disk(&usage);
    let days = if is_in_trial {
        Some(trial_days_left(&usage))
    } else {
        None
    };
    let trial_ends_at = usage.trial_started_at.map(|s| s + TRIAL_DAYS * 86_400);

    UsageStats {
        is_pro,
        is_in_trial,
        trial_days_left: days,
        trial_started_at: usage.trial_started_at,
        trial_ends_at,
        polish_count_this_month: polish_count_this_month(&usage),
        dictionary_count: crate::dictionary::get_dictionary().len(),
        history_count: crate::history::get_history().len(),
    }
}

/// Exposed to the frontend so the Settings → Analytics section can render
/// rolling-window totals (this week, this month, all time) plus a sparse
/// daily series for the last 30 days. Pure read of `usage.json`, no network.
#[tauri::command]
pub fn get_analytics_summary() -> AnalyticsSummary {
    analytics_summary()
}

/// Initialize trial on first launch (idempotent).
pub fn init() {
    let _ = start_trial_if_needed();
}
