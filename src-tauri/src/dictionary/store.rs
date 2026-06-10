// TTP - Talk To Paste
// Dictionary persistence layer - stores learned corrections in JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// In-memory cache so get_dictionary() doesn't re-read + re-parse the JSON
// file on every Settings render or every transcription pass (apply_dictionary
// is on the hot path). Invalidated on writes.
static DICTIONARY_CACHE: OnceLock<Mutex<Option<(Vec<DictionaryEntry>, Instant)>>> = OnceLock::new();
const CACHE_TTL_SECS: u64 = 5;

fn cache() -> &'static Mutex<Option<(Vec<DictionaryEntry>, Instant)>> {
    DICTIONARY_CACHE.get_or_init(|| Mutex::new(None))
}

fn read_cached() -> Option<Vec<DictionaryEntry>> {
    let guard = cache().lock().ok()?;
    let (entries, cached_at) = guard.as_ref()?;
    if cached_at.elapsed().as_secs() < CACHE_TTL_SECS {
        Some(entries.clone())
    } else {
        None
    }
}

fn store_cache(entries: Vec<DictionaryEntry>) {
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((entries, Instant::now()));
    }
}

fn invalidate_cache() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

/// A single dictionary entry mapping original (misheard) text to correction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    /// The original (incorrect) transcription
    pub original: String,
    /// The user's correction
    pub correction: String,
    /// Unix timestamp when entry was created
    pub created_at: i64,
}

/// Get the path to the dictionary JSON file
/// Location: ~/.config/ttp/dictionary.json (cross-platform via dirs crate)
fn get_dictionary_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?;

    let ttp_dir = config_dir.join("ttp");

    // Ensure directory exists
    if !ttp_dir.exists() {
        fs::create_dir_all(&ttp_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    Ok(ttp_dir.join("dictionary.json"))
}

/// Load all dictionary entries from file
/// Returns empty Vec if file doesn't exist or is empty
#[tauri::command]
pub fn get_dictionary() -> Vec<DictionaryEntry> {
    if let Some(cached) = read_cached() {
        return cached;
    }

    let path = match get_dictionary_path() {
        Ok(p) => p,
        Err(e) => {
            crate::logging::log_error(&format!("[Dictionary] Failed to get path: {}", e));
            return Vec::new();
        }
    };

    if !path.exists() {
        store_cache(Vec::new());
        return Vec::new();
    }

    let mut file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            crate::logging::log_error(&format!("[Dictionary] Failed to open file: {}", e));
            return Vec::new();
        }
    };

    let mut contents = String::new();
    if let Err(e) = file.read_to_string(&mut contents) {
        crate::logging::log_error(&format!("[Dictionary] Failed to read file: {}", e));
        return Vec::new();
    }

    if contents.trim().is_empty() {
        store_cache(Vec::new());
        return Vec::new();
    }

    let entries: Vec<DictionaryEntry> = match serde_json::from_str(&contents) {
        Ok(entries) => entries,
        Err(e) => {
            // Parse failure on a non-empty file = corruption. Quarantine the
            // broken file so (a) the user can find and inspect it, (b) it
            // doesn't get silently overwritten next save, masking the bug.
            // Without this, dictionary.json corruption shows up as "all my
            // custom corrections vanished" with zero forensic trail.
            crate::logging::log_error(&format!(
                "[Dictionary] Corrupt JSON ({} bytes): {}. Quarantining as .corrupt.",
                contents.len(),
                e
            ));
            let quarantine = path.with_extension("json.corrupt");
            let _ = fs::rename(&path, &quarantine);
            Vec::new()
        }
    };

    store_cache(entries.clone());
    entries
}

/// Tauri command to add a dictionary entry from the frontend
#[tauri::command]
pub fn add_dictionary_entry(original: String, correction: String) -> Result<(), String> {
    add_entry(&original, &correction)
}

/// Add a new entry to the dictionary
/// If an entry with the same original text exists, it will be updated.
/// Free tier: blocks NEW entries when at the cap (existing entries stay editable).
pub fn add_entry(original: &str, correction: &str) -> Result<(), String> {
    let path = get_dictionary_path()?;

    // Load existing entries
    let mut entries = get_dictionary();

    // Check if entry already exists (case-insensitive update)
    let original_lower = original.to_lowercase();
    let existing_idx = entries
        .iter()
        .position(|e| e.original.to_lowercase() == original_lower);

    // Free tier cap — only blocks brand-new entries; updates pass through.
    if existing_idx.is_none()
        && !crate::licensing::is_pro_or_trial_disk()
        && entries.len() >= crate::licensing::FREE_DICTIONARY_LIMIT
    {
        return Err(format!(
            "Free tier limit reached ({} dictionary entries). Upgrade to TTP Pro for unlimited.",
            crate::licensing::FREE_DICTIONARY_LIMIT
        ));
    }

    let timestamp = chrono::Utc::now().timestamp();
    let new_entry = DictionaryEntry {
        original: original.to_string(),
        correction: correction.to_string(),
        created_at: timestamp,
    };

    if let Some(idx) = existing_idx {
        entries[idx] = new_entry;
    } else {
        entries.push(new_entry);
    }

    // Write back to file
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize dictionary: {}", e))?;

    let mut file = fs::File::create(&path)
        .map_err(|e| format!("Failed to create dictionary file: {}", e))?;

    file.write_all(json.as_bytes())
        .map_err(|e| format!("Failed to write dictionary file: {}", e))?;

    // Refresh cache with what we just wrote so subsequent get_dictionary()
    // calls (and apply_dictionary on the hot path) skip disk.
    store_cache(entries);

    Ok(())
}

/// Delete an entry from the dictionary by its original text
#[tauri::command]
pub fn delete_dictionary_entry(original: String) -> Result<(), String> {
    delete_entry_internal(&original)
}

/// Internal function to delete an entry
fn delete_entry_internal(original: &str) -> Result<(), String> {
    let path = get_dictionary_path()?;

    // Load existing entries
    let mut entries = get_dictionary();

    // Find and remove entry (case-insensitive)
    let original_len = entries.len();
    let original_lower = original.to_lowercase();
    entries.retain(|e| e.original.to_lowercase() != original_lower);

    if entries.len() == original_len {
        return Err(format!("Entry not found: {}", original));
    }

    // Write back to file (or delete if empty)
    if entries.is_empty() {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete dictionary file: {}", e))?;
        }
    } else {
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("Failed to serialize dictionary: {}", e))?;

        let mut file = fs::File::create(&path)
            .map_err(|e| format!("Failed to create dictionary file: {}", e))?;

        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write dictionary file: {}", e))?;
    }

    store_cache(entries);

    Ok(())
}

/// Apply dictionary corrections to text as hard replacements
///
/// Convenience wrapper that pulls the live dictionary via the cache and
/// dispatches to `apply_dictionary_to_text`. Production hot path.
pub fn apply_dictionary(text: &str) -> String {
    let entries = get_dictionary();
    apply_dictionary_to_text(text, &entries)
}

/// Pure-function core of `apply_dictionary`. Takes the entries explicitly
/// so the unit tests don't have to fake a global cache.
///
/// Rules:
///   * Case-insensitive original match.
///   * Word boundaries on both sides — we don't substitute inside other
///     words ("api" -> "API" must not turn "rapidly" into "rAPIdly").
///   * Boundary check uses the underlying byte's ASCII-alphanumeric class
///     since `is_ascii_alphanumeric` is well-defined on every byte.
///     Non-ASCII bytes (lead/trail bytes of multi-byte UTF-8 code points)
///     return false from `is_ascii_alphanumeric`, so they're treated as
///     boundaries — which is the conservative default for unknown scripts.
///
/// Audit note: a previous version of the regex was byte-indexed inside
/// `result`, which broke when `original.to_lowercase()` had a different
/// byte length than `original` (e.g. Turkish dotless-i). The current code
/// uses byte indices of `original.len()` (the original, NOT the lowered
/// form) consistently, so byte arithmetic stays correct even when lower-
/// casing changes width.
pub fn apply_dictionary_to_text(text: &str, entries: &[DictionaryEntry]) -> String {
    if entries.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();
    for entry in entries {
        let original_lower = entry.original.to_lowercase();
        if original_lower.is_empty() {
            continue;
        }
        let mut new_result = String::new();
        let mut remaining = result.as_str();

        while !remaining.is_empty() {
            let Some(pos) = remaining.to_lowercase().find(&original_lower) else {
                new_result.push_str(remaining);
                break;
            };

            // Word boundary on the byte before `pos` (start of the match).
            let before_ok = pos == 0
                || !remaining.as_bytes()[pos - 1].is_ascii_alphanumeric();
            // Word boundary on the byte AFTER the match. Using
            // `entry.original.len()` (the SOURCE byte length) keeps the
            // indices correct even when lowercasing changes byte width.
            let after_pos = pos + entry.original.len();
            let after_ok = after_pos >= remaining.len()
                || !remaining.as_bytes()[after_pos].is_ascii_alphanumeric();

            if before_ok && after_ok {
                new_result.push_str(&remaining[..pos]);
                new_result.push_str(&entry.correction);
                remaining = &remaining[after_pos..];
            } else {
                // Not a word boundary match — keep the original text up to
                // and including this hit, then continue scanning past it.
                new_result.push_str(&remaining[..pos + entry.original.len()]);
                remaining = &remaining[pos + entry.original.len()..];
            }
        }

        result = new_result;
    }

    result
}

/// Clear all dictionary entries (delete file)
#[tauri::command]
pub fn clear_dictionary() -> Result<(), String> {
    invalidate_cache();

    let path = get_dictionary_path()?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete dictionary file: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod apply_dictionary_tests {
    use super::*;

    fn e(original: &str, correction: &str) -> DictionaryEntry {
        DictionaryEntry {
            original: original.into(),
            correction: correction.into(),
            created_at: 0,
        }
    }

    #[test]
    fn empty_entries_returns_input_unchanged() {
        assert_eq!(
            apply_dictionary_to_text("hello world", &[]),
            "hello world"
        );
    }

    #[test]
    fn empty_text_returns_empty() {
        assert_eq!(
            apply_dictionary_to_text("", &[e("foo", "bar")]),
            ""
        );
    }

    #[test]
    fn case_insensitive_match_preserves_correction_casing() {
        let entries = [e("api", "API")];
        assert_eq!(
            apply_dictionary_to_text("call the api endpoint", &entries),
            "call the API endpoint"
        );
        assert_eq!(
            apply_dictionary_to_text("call the API endpoint", &entries),
            "call the API endpoint"
        );
        assert_eq!(
            apply_dictionary_to_text("call the Api endpoint", &entries),
            "call the API endpoint"
        );
    }

    #[test]
    fn does_not_match_inside_other_words() {
        // "rapidly" contains "api" but it must NOT be substituted because
        // there is an alphanumeric character on both sides.
        let entries = [e("api", "API")];
        assert_eq!(
            apply_dictionary_to_text("words rapidly translate", &entries),
            "words rapidly translate"
        );
    }

    #[test]
    fn matches_at_start_and_end_of_string() {
        let entries = [e("api", "API")];
        assert_eq!(apply_dictionary_to_text("api at start", &entries), "API at start");
        assert_eq!(apply_dictionary_to_text("end with api", &entries), "end with API");
        assert_eq!(apply_dictionary_to_text("api", &entries), "API");
    }

    #[test]
    fn matches_around_punctuation_boundaries() {
        let entries = [e("api", "API")];
        // Comma, period, parens — none ASCII-alphanumeric, all word boundaries.
        assert_eq!(
            apply_dictionary_to_text("the (api), as before", &entries),
            "the (API), as before"
        );
    }

    #[test]
    fn replaces_multiple_occurrences_in_one_pass() {
        let entries = [e("api", "API")];
        assert_eq!(
            apply_dictionary_to_text("api and api and api", &entries),
            "API and API and API"
        );
    }

    #[test]
    fn applies_multiple_entries_in_order() {
        // Both replacements should fire. Order matters only if one entry's
        // output contains another entry's source — not the case here.
        let entries = [e("supa base", "Supabase"), e("api", "API")];
        assert_eq!(
            apply_dictionary_to_text("call the api and check supa base", &entries),
            "call the API and check Supabase"
        );
    }

    #[test]
    fn idempotent_when_applied_twice() {
        let entries = [e("api", "API")];
        let pass1 = apply_dictionary_to_text("call the API endpoint", &entries);
        let pass2 = apply_dictionary_to_text(&pass1, &entries);
        assert_eq!(pass1, pass2);
    }

    #[test]
    fn handles_non_ascii_payload_without_panic() {
        // Multi-byte characters in the surrounding context. The function
        // must not panic on byte-index arithmetic — `is_ascii_alphanumeric`
        // is well-defined on every byte, and the lead/trail bytes of UTF-8
        // sequences return false (treated as word boundaries).
        let entries = [e("api", "API")];
        let out = apply_dictionary_to_text("français api caractère", &entries);
        assert_eq!(out, "français API caractère");
    }

    #[test]
    fn empty_original_entry_is_skipped() {
        // A malformed entry with an empty original would otherwise match
        // EVERYWHERE in the input — explicitly guarded.
        let entries = [e("", "X"), e("api", "API")];
        assert_eq!(
            apply_dictionary_to_text("the api", &entries),
            "the API"
        );
    }

    #[test]
    fn entry_with_punctuation_in_original_matches_literally() {
        // No special regex meaning — periods are matched as bytes.
        let entries = [e("u.s.a.", "USA")];
        assert_eq!(
            apply_dictionary_to_text("the u.s.a. and others", &entries),
            "the USA and others"
        );
    }

    #[test]
    fn multi_word_original_is_supported() {
        let entries = [e("machine learning", "ML")];
        assert_eq!(
            apply_dictionary_to_text("study machine learning today", &entries),
            "study ML today"
        );
    }
}
