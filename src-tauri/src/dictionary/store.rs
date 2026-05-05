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
            eprintln!("[Dictionary] Failed to get path: {}", e);
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
            eprintln!("[Dictionary] Failed to open file: {}", e);
            return Vec::new();
        }
    };

    let mut contents = String::new();
    if let Err(e) = file.read_to_string(&mut contents) {
        eprintln!("[Dictionary] Failed to read file: {}", e);
        return Vec::new();
    }

    if contents.trim().is_empty() {
        store_cache(Vec::new());
        return Vec::new();
    }

    let entries: Vec<DictionaryEntry> = match serde_json::from_str(&contents) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("[Dictionary] Failed to parse JSON: {}", e);
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
/// This is a post-processing step that guarantees dictionary entries
/// are applied regardless of whether the LLM honored them.
/// Uses case-insensitive word-boundary matching.
pub fn apply_dictionary(text: &str) -> String {
    let entries = get_dictionary();
    if entries.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();
    for entry in &entries {
        // Case-insensitive replacement preserving word boundaries
        // We search for the original word and replace with correction
        let original_lower = entry.original.to_lowercase();
        let mut new_result = String::new();
        let mut remaining = result.as_str();

        while !remaining.is_empty() {
            // Find next occurrence (case-insensitive)
            if let Some(pos) = remaining.to_lowercase().find(&original_lower) {
                // Check word boundaries
                let before_ok = pos == 0
                    || !remaining.as_bytes()[pos - 1].is_ascii_alphanumeric();
                let after_pos = pos + entry.original.len();
                let after_ok = after_pos >= remaining.len()
                    || !remaining.as_bytes()[after_pos].is_ascii_alphanumeric();

                if before_ok && after_ok {
                    new_result.push_str(&remaining[..pos]);
                    new_result.push_str(&entry.correction);
                    remaining = &remaining[after_pos..];
                } else {
                    // Not a word boundary match, skip past this occurrence
                    new_result.push_str(&remaining[..pos + entry.original.len()]);
                    remaining = &remaining[pos + entry.original.len()..];
                }
            } else {
                new_result.push_str(remaining);
                break;
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
