// TTP - Talk To Paste
// Dictionary persistence layer - stores learned corrections in JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

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
    let path = match get_dictionary_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[Dictionary] Failed to get path: {}", e);
            return Vec::new();
        }
    };

    if !path.exists() {
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
        return Vec::new();
    }

    match serde_json::from_str(&contents) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("[Dictionary] Failed to parse JSON: {}", e);
            Vec::new()
        }
    }
}

/// Add a new entry to the dictionary
/// If an entry with the same original text exists, it will be updated
pub fn add_entry(original: &str, correction: &str) -> Result<(), String> {
    let path = get_dictionary_path()?;

    // Load existing entries
    let mut entries = get_dictionary();

    // Check if entry already exists (update if so)
    let existing_idx = entries.iter().position(|e| e.original == original);

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

    println!("[Dictionary] Added entry: {} -> {}", original, correction);
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

    // Find and remove entry
    let original_len = entries.len();
    entries.retain(|e| e.original != original);

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

    println!("[Dictionary] Deleted entry: {}", original);
    Ok(())
}

/// Clear all dictionary entries (delete file)
#[tauri::command]
pub fn clear_dictionary() -> Result<(), String> {
    let path = get_dictionary_path()?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete dictionary file: {}", e))?;
    }

    println!("[Dictionary] Cleared all entries");
    Ok(())
}
