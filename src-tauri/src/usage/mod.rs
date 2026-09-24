// TTP - Talk To Paste
// Usage tracking - polish counter (monthly), analytics

mod store;

pub use store::{
    AnalyticsSummary, analytics_summary, load_usage,
    polish_count_this_month, record_polish_success, record_transcription,
    warm_keychain_cache,
};

use serde::Serialize;

use crate::licensing;

/// Snapshot of usage state, exposed to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct UsageStats {
    pub is_pro: bool,
    // No trial fields. The trial is gone (see `licensing`), and a countdown
    // field left here would be a countdown the UI could render again.
    //
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
    UsageStats {
        is_pro: licensing::is_pro_disk(),
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
