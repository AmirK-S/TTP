// TTP - Talk To Paste
// Usage tracking - polish counter (monthly), trial state

mod store;

pub use store::{
    AnalyticsSummary, UsageRecord, analytics_summary, current_month_key, load_usage,
    polish_count_this_month, record_polish_success, record_transcription, save_usage,
    should_notify_polish_cap_once_this_month, start_trial_if_needed, trial_days_left,
    trial_started_at,
};

use serde::Serialize;

use crate::licensing;

/// Snapshot of usage state, exposed to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct UsageStats {
    pub is_pro: bool,
    pub is_in_trial: bool,
    pub trial_days_left: Option<i64>,
    pub trial_started_at: Option<i64>,
    pub polish_count_this_month: u32,
    pub polish_limit_free: u32,
    pub dictionary_count: usize,
    pub dictionary_limit_free: usize,
    pub history_count: usize,
    pub history_limit_free: usize,
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

    UsageStats {
        is_pro,
        is_in_trial,
        trial_days_left: days,
        trial_started_at: usage.trial_started_at,
        polish_count_this_month: polish_count_this_month(&usage),
        polish_limit_free: licensing::FREE_POLISH_PER_MONTH,
        dictionary_count: crate::dictionary::get_dictionary().len(),
        dictionary_limit_free: licensing::FREE_DICTIONARY_LIMIT,
        history_count: crate::history::get_history().len(),
        history_limit_free: licensing::FREE_HISTORY_LIMIT,
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
