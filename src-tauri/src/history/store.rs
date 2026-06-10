// TTP - Talk To Paste
// History store - handles transcription history persistence to JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// In-memory cache so opening Settings (which calls get_history) doesn't
// re-read + re-parse the JSON file on every render. Invalidated on writes.
static HISTORY_CACHE: OnceLock<Mutex<Option<(Vec<HistoryEntry>, Instant)>>> = OnceLock::new();
const CACHE_TTL_SECS: u64 = 5;

fn cache() -> &'static Mutex<Option<(Vec<HistoryEntry>, Instant)>> {
    HISTORY_CACHE.get_or_init(|| Mutex::new(None))
}

fn read_cached() -> Option<Vec<HistoryEntry>> {
    let guard = cache().lock().ok()?;
    let (entries, cached_at) = guard.as_ref()?;
    if cached_at.elapsed().as_secs() < CACHE_TTL_SECS {
        Some(entries.clone())
    } else {
        None
    }
}

fn store_cache(entries: Vec<HistoryEntry>) {
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((entries, Instant::now()));
    }
}

fn invalidate_cache() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

/// A single history entry representing a past transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The polished/final transcription text
    pub text: String,
    /// Unix timestamp in milliseconds when transcription was created
    pub timestamp: i64,
    /// The raw transcription text before AI polish (if polish was enabled)
    pub raw_text: Option<String>,
}

/// Get the history file path (~/.config/ttp/history.json)
fn get_history_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ttp").join("history.json"))
}

/// Load history from file, return empty vec if file doesn't exist
/// Returns entries sorted by timestamp, newest first
#[tauri::command]
pub fn get_history() -> Vec<HistoryEntry> {
    if let Some(cached) = read_cached() {
        return cached;
    }

    let Some(path) = get_history_path() else {
        return Vec::new();
    };

    if !path.exists() {
        store_cache(Vec::new());
        return Vec::new();
    }

    let entries = match fs::read_to_string(&path) {
        Ok(content) => {
            let mut entries: Vec<HistoryEntry> =
                serde_json::from_str(&content).unwrap_or_default();
            entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            entries
        }
        Err(_) => Vec::new(),
    };

    store_cache(entries.clone());
    entries
}

/// Add a new entry to history
/// Prepends to existing history (newest first).
/// Free tier: skips silently when at the cap (existing entries are grandfathered).
pub fn add_history_entry(text: &str, raw_text: Option<&str>) -> Result<(), String> {
    let path = get_history_path().ok_or("Could not determine config directory")?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Load existing history
    let mut entries = get_history();

    // Free tier cap — silently skip new entries (don't fail the whole pipeline).
    if !crate::licensing::is_pro_or_trial_disk()
        && entries.len() >= crate::licensing::FREE_HISTORY_LIMIT
    {
        return Ok(());
    }

    // Create new entry with current timestamp
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let entry = HistoryEntry {
        text: text.to_string(),
        timestamp,
        raw_text: raw_text.map(|s| s.to_string()),
    };

    // Prepend new entry (will be at start after sort)
    entries.insert(0, entry);

    // Enforce maximum history size to prevent unbounded growth
    const MAX_HISTORY_ENTRIES: usize = 500;
    entries.truncate(MAX_HISTORY_ENTRIES);

    // Save back to file
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write history file: {}", e))?;

    // Refresh the cache with what we just persisted, so the next get_history()
    // doesn't have to re-read disk.
    store_cache(entries);

    Ok(())
}

/// Re-type a stored history entry into the currently focused app.
///
/// Used by Settings → History → "Use again" so the user can re-insert a
/// past transcription without copy-pasting through the clipboard. We
/// type directly via the same code path the pipeline uses for fresh
/// transcriptions ≤2000 chars; longer entries fall back to the
/// clipboard+Cmd+V path.
///
/// Threshold is hard-coded here (matches the pipeline's
/// DIRECT_TYPING_MAX_CHARS) so the Settings command has the same
/// guarantee as a live transcription.
#[tauri::command]
pub async fn replay_history_entry(text: String, app: tauri::AppHandle) -> Result<(), String> {
    const DIRECT_TYPING_MAX_CHARS: usize = 2_000;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("error.history_empty_text".to_string());
    }

    // Accessibility / mic permission semantics are exactly the same as the
    // live transcription paste path. If the user has revoked Accessibility
    // since they granted it, simulate_typing / simulate_paste both surface
    // a clear error which the JS side maps to error.paste_failed.
    let use_direct_typing = trimmed.chars().count() <= DIRECT_TYPING_MAX_CHARS;
    if use_direct_typing {
        // Run on the Tauri blocking pool so a slow target (Slack, Mail)
        // can't pin the IPC thread.
        let owned = trimmed.to_string();
        tauri::async_runtime::spawn_blocking(move || crate::paste::simulate_typing(&owned))
            .await
            .map_err(|e| format!("typing task panicked: {}", e))?
    } else {
        // Long entries go via clipboard + Cmd+V. We restore the user's
        // prior clipboard exactly like the live transcription pipeline so
        // a Replay never silently overwrites whatever the user had copied.
        use crate::paste::ClipboardGuard;
        let guard = ClipboardGuard::new(&app);
        guard
            .write_text(trimmed)
            .map_err(|e| format!("Clipboard write failed: {}", e))?;
        tauri::async_runtime::spawn_blocking(crate::paste::simulate_paste)
            .await
            .map_err(|e| format!("paste task panicked: {}", e))??;
        // Best-effort restore — same delay constant as the pipeline so a
        // slow Electron target finishes reading the pasteboard before we
        // overwrite it. 1500 ms is the worst observed Electron pause.
        tokio::time::sleep(std::time::Duration::from_millis(1_500)).await;
        let _ = guard.restore();
        Ok(())
    }
}

/// Clear all history by deleting the history file
#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    invalidate_cache();

    let Some(path) = get_history_path() else {
        return Ok(()); // No config dir, nothing to clear
    };

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete history file: {}", e))?;
    }

    Ok(())
}
