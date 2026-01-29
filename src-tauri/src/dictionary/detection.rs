// TTP - Talk To Paste
// Correction detection - monitors for user corrections after paste
//
// Implements a 10-second detection window after paste to capture proper noun corrections.
// Only learns clear substitutions of proper nouns (capitalized words not at sentence start).

use super::store::add_entry;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::time::{sleep, Duration};

/// Correction detection window duration (10 seconds)
const DETECTION_WINDOW_SECS: u64 = 10;

/// Start a correction detection window after paste
///
/// Spawns an async task that waits 10 seconds, then checks if the clipboard
/// contains a corrected version of the pasted text. If a proper noun correction
/// is detected, it's added to the dictionary.
///
/// # Arguments
/// * `app` - Tauri app handle for clipboard access
/// * `pasted_text` - The text that was just pasted
pub fn start_correction_window(app: &AppHandle, pasted_text: String) {
    let app_clone = app.clone();

    // Spawn background task for detection
    tokio::spawn(async move {
        // Wait for detection window
        sleep(Duration::from_secs(DETECTION_WINDOW_SECS)).await;

        // Read current clipboard content
        let clipboard_text = match app_clone.clipboard().read_text() {
            Ok(text) => text,
            Err(e) => {
                eprintln!("[Detection] Failed to read clipboard: {}", e);
                return;
            }
        };

        // Compare and detect corrections
        if let Some((original, correction)) = detect_proper_noun_correction(&pasted_text, &clipboard_text) {
            println!("[Detection] Found correction: {} -> {}", original, correction);
            if let Err(e) = add_entry(&original, &correction) {
                eprintln!("[Detection] Failed to add dictionary entry: {}", e);
            }
        }
    });
}

/// Detect if clipboard contains a proper noun correction of the pasted text
///
/// Compares word-by-word, looking for single-word substitutions where:
/// - The new word starts with a capital letter
/// - The word is not at the start of a sentence (to avoid sentence-start capitalization)
/// - The change is not just a case difference at sentence start
///
/// # Returns
/// * `Some((original, correction))` if a proper noun correction is detected
/// * `None` if no correction found or clipboard is unrelated
fn detect_proper_noun_correction(original: &str, corrected: &str) -> Option<(String, String)> {
    // Quick check: if texts are identical, no correction
    if original == corrected {
        return None;
    }

    // Split into words while preserving position info
    let original_words: Vec<&str> = original.split_whitespace().collect();
    let corrected_words: Vec<&str> = corrected.split_whitespace().collect();

    // If word counts differ significantly, this isn't a simple correction
    // Allow small differences (user might have added/removed a word)
    if (original_words.len() as i32 - corrected_words.len() as i32).abs() > 2 {
        return None;
    }

    // Find word substitutions
    let mut corrections: Vec<(String, String)> = Vec::new();

    // Use longest common subsequence approach for alignment
    // For simplicity, we'll do direct comparison when lengths match
    if original_words.len() == corrected_words.len() {
        for (i, (orig, corr)) in original_words.iter().zip(corrected_words.iter()).enumerate() {
            if orig != corr && is_proper_noun_correction(orig, corr, i == 0) {
                corrections.push((orig.to_string(), corr.to_string()));
            }
        }
    }

    // Return only if we found exactly one proper noun correction
    // Multiple corrections might indicate the text is unrelated
    if corrections.len() == 1 {
        return Some(corrections.into_iter().next().unwrap());
    }

    None
}

/// Check if a word change looks like a proper noun correction
///
/// # Arguments
/// * `original` - The original word
/// * `corrected` - The corrected word
/// * `is_sentence_start` - Whether this word is at the start of a sentence
///
/// # Returns
/// * `true` if this looks like a proper noun correction
fn is_proper_noun_correction(original: &str, corrected: &str, is_sentence_start: bool) -> bool {
    // Strip punctuation for comparison
    let orig_clean = original.trim_matches(|c: char| c.is_ascii_punctuation());
    let corr_clean = corrected.trim_matches(|c: char| c.is_ascii_punctuation());

    // Both must be non-empty
    if orig_clean.is_empty() || corr_clean.is_empty() {
        return false;
    }

    // Correction must start with uppercase
    let corr_starts_upper = corr_clean.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
    if !corr_starts_upper {
        return false;
    }

    // If at sentence start, ignore case-only changes (normal capitalization)
    if is_sentence_start {
        let orig_lower = orig_clean.to_lowercase();
        let corr_lower = corr_clean.to_lowercase();
        if orig_lower == corr_lower {
            // Just a case change at sentence start - not a proper noun
            return false;
        }
    }

    // Check if original was lowercase (misheard proper noun)
    let orig_starts_lower = orig_clean.chars().next().map(|c| c.is_lowercase()).unwrap_or(false);

    // The word should be similar enough (same or similar spelling)
    // Proper noun corrections are usually phonetically similar
    let similarity = calculate_similarity(orig_clean, corr_clean);

    // If original was lowercase and correction is uppercase with some similarity, it's a proper noun correction
    // Also accept when both are capitalized but spelled differently (different proper noun)
    orig_starts_lower && similarity > 0.3 || similarity > 0.5
}

/// Calculate a simple similarity score between two strings
/// Returns a value between 0.0 (completely different) and 1.0 (identical)
fn calculate_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    if a_lower == b_lower {
        return 1.0;
    }

    // Simple Levenshtein-inspired approach
    let a_chars: Vec<char> = a_lower.chars().collect();
    let b_chars: Vec<char> = b_lower.chars().collect();

    let max_len = a_chars.len().max(b_chars.len()) as f64;
    if max_len == 0.0 {
        return 1.0;
    }

    // Count matching characters at same position
    let matches = a_chars.iter().zip(b_chars.iter()).filter(|(a, b)| a == b).count() as f64;

    // Simple similarity: matches / max_length
    matches / max_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_proper_noun_correction() {
        // Basic proper noun correction
        let result = detect_proper_noun_correction(
            "I met john at the cafe",
            "I met John at the cafe"
        );
        assert_eq!(result, Some(("john".to_string(), "John".to_string())));

        // No correction
        let result = detect_proper_noun_correction(
            "Hello world",
            "Hello world"
        );
        assert_eq!(result, None);

        // Sentence start capitalization - should ignore
        let result = detect_proper_noun_correction(
            "hello world",
            "Hello world"
        );
        assert_eq!(result, None);

        // Multiple changes - should ignore (likely unrelated text)
        let result = detect_proper_noun_correction(
            "I met john and mary",
            "I met John and Mary"
        );
        // This returns None because we only accept single corrections
        assert_eq!(result, None);
    }

    #[test]
    fn test_similarity() {
        assert!(calculate_similarity("john", "John") == 1.0);
        assert!(calculate_similarity("jon", "John") > 0.5);
        assert!(calculate_similarity("abc", "xyz") < 0.3);
    }
}
