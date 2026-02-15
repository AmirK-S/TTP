// TTP - Talk To Paste
// Correction detection - monitors for user corrections after paste
//
// After pasting, waits 10 seconds then reads back the focused text field
// using macOS Accessibility API. Compares with original to detect corrections.

use super::store::add_entry;
use crate::paste::read_focused_text;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};

/// How often to poll the focused text field (milliseconds)
const POLL_INTERVAL_MS: u64 = 500;

/// Total detection window duration (seconds)
const DETECTION_WINDOW_SECS: u64 = 15;

/// Start a correction detection window after paste
///
/// Spawns an async task that polls the focused text field every second
/// via Accessibility API. As soon as a correction is detected, it's saved
/// to the dictionary and polling stops.
///
/// # Arguments
/// * `_app` - Tauri app handle (kept for future use)
/// * `pasted_text` - The text that was just pasted
pub fn start_correction_window(app: &AppHandle, pasted_text: String) {
    let app_handle = app.clone();
    // Spawn background task for detection
    tokio::spawn(async move {
        // Small initial delay to let the paste settle
        sleep(Duration::from_millis(500)).await;

        let polls = (DETECTION_WINDOW_SECS * 1000) / POLL_INTERVAL_MS;

        for poll in 0..polls {
            // Read the current text from the focused UI element
            let current_text = match tokio::task::spawn_blocking(read_focused_text).await {
                Ok(Some(text)) => text,
                Ok(None) => {
                    // Field not readable (user switched app, etc.) — stop polling
                    break;
                }
                Err(_) => break,
            };

            // Compare with pasted text
            if let Some(corrections) = detect_corrections(&pasted_text, &current_text) {
                let mut added = false;
                for (original, correction) in &corrections {
                    if let Err(e) = add_entry(original, correction) {
                        eprintln!("[Detection] Failed to add dictionary entry: {}", e);
                    } else {
                        added = true;
                    }
                }
                // Notify frontend that dictionary changed
                if added {
                    let _ = app_handle.emit("dictionary-changed", ());
                }
                // Done — corrections found, stop polling
                return;
            }

            // Wait before next poll
            sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        }

    });
}

/// Check if a word change looks like a real spelling correction vs noise
///
/// A real correction is: "Grok" → "Groq", "john" → "John", "parris" → "Paris"
/// NOT a correction: "donne" → "donne.Magnifique", "marche" → "ma"
fn is_valid_correction(orig: &str, curr: &str) -> bool {
    let orig_lower = orig.to_lowercase();
    let curr_lower = curr.to_lowercase();

    // Both words must be at least 2 characters
    if orig.len() < 2 || curr.len() < 2 {
        return false;
    }

    // Reject if one is a proper substring of the other (user concatenated text)
    // e.g. "donne" → "donne.Magnifique" or "test" → "testing"
    // But allow case-only changes like "Kellou" → "KELLOU" (same length)
    if orig_lower != curr_lower
        && (curr_lower.contains(&orig_lower) || orig_lower.contains(&curr_lower))
    {
        return false;
    }

    // Length ratio check: correction should be similar length (within 2x)
    // Real corrections don't drastically change word length
    let len_ratio = orig.len().max(curr.len()) as f64 / orig.len().min(curr.len()) as f64;
    if len_ratio > 2.0 {
        return false;
    }

    // Similarity must be at least 0.5 (stricter than before)
    let sim = calculate_similarity(orig, curr);
    if sim < 0.5 {
        return false;
    }

    true
}

/// Detect corrections by comparing original pasted text with current field content
///
/// The pasted text may be a substring of the field content (if the field already
/// had text before paste). We find the best matching region and compare word-by-word.
///
/// Only detects real spelling corrections (similar length, high similarity, not
/// substring concatenation). Ignores text added after the paste.
///
/// Returns a list of (original, correction) pairs, or None if no corrections found.
fn detect_corrections(pasted: &str, current: &str) -> Option<Vec<(String, String)>> {
    // Quick check: if they're identical, no correction
    if pasted == current {
        return None;
    }

    let pasted_words: Vec<&str> = pasted.split_whitespace().collect();
    let current_words: Vec<&str> = current.split_whitespace().collect();

    if pasted_words.is_empty() {
        return None;
    }

    // Find the best alignment of pasted text within current text
    let alignment = find_best_alignment(&pasted_words, &current_words);

    let (offset, aligned_len) = match alignment {
        Some(a) => a,
        None => {
            return None;
        }
    };

    // Compare aligned words
    let mut corrections = Vec::new();
    if current_words.len() >= pasted_words.len() {
        // Normal case: compare pasted words against aligned region in current
        for i in 0..aligned_len {
            let orig = pasted_words[i];
            let curr = current_words[offset + i];

            let orig_clean = orig.trim_matches(|c: char| c.is_ascii_punctuation());
            let curr_clean = curr.trim_matches(|c: char| c.is_ascii_punctuation());

            if orig_clean != curr_clean && !orig_clean.is_empty() && !curr_clean.is_empty() {
                if is_valid_correction(orig_clean, curr_clean) {
                    corrections.push((orig_clean.to_string(), curr_clean.to_string()));
                }
            }
        }
    } else {
        // Current is shorter: compare current words against aligned region in pasted
        for i in 0..aligned_len {
            let orig = pasted_words[offset + i];
            let curr = current_words[i];

            let orig_clean = orig.trim_matches(|c: char| c.is_ascii_punctuation());
            let curr_clean = curr.trim_matches(|c: char| c.is_ascii_punctuation());

            if orig_clean != curr_clean && !orig_clean.is_empty() && !curr_clean.is_empty() {
                if is_valid_correction(orig_clean, curr_clean) {
                    corrections.push((orig_clean.to_string(), curr_clean.to_string()));
                }
            }
        }
    }

    if corrections.is_empty() {
        None
    } else {
        Some(corrections)
    }
}

/// Find the best alignment of pasted words within the current text
///
/// Returns (offset, length) where offset is the start index in current_words
/// and length is the number of words to compare (min of both lengths).
fn find_best_alignment(pasted: &[&str], current: &[&str]) -> Option<(usize, usize)> {
    if pasted.is_empty() || current.is_empty() {
        return None;
    }

    // If same length, assume direct alignment
    if pasted.len() == current.len() {
        return Some((0, pasted.len()));
    }

    // Use the shorter length as window size to handle word additions/deletions
    let window_len = pasted.len().min(current.len());

    // If current text is shorter (user deleted words), search pasted within current
    // If current text is longer (field had prior text), search pasted within current
    let (search_in, search_for, search_len) = if current.len() >= pasted.len() {
        (current, pasted, pasted.len())
    } else {
        // Current is shorter — user may have deleted words.
        // Still try to align using the shorter window
        (current, pasted, current.len())
    };

    if search_in.len() < search_len {
        return None;
    }

    let mut best_offset = 0;
    let mut best_score = 0usize;

    for offset in 0..=(search_in.len() - search_len) {
        let score: usize = search_for
            .iter()
            .take(search_len)
            .zip(&search_in[offset..offset + search_len])
            .filter(|(a, b)| {
                let a_clean = a.trim_matches(|c: char| c.is_ascii_punctuation());
                let b_clean = b.trim_matches(|c: char| c.is_ascii_punctuation());
                a_clean.eq_ignore_ascii_case(b_clean)
            })
            .count();

        if score > best_score {
            best_score = score;
            best_offset = offset;
        }
    }

    // Require at least 50% of the window to match for a valid alignment
    if best_score >= window_len / 2 {
        Some((best_offset, search_len))
    } else {
        None
    }
}

/// Calculate similarity between two strings using normalized Levenshtein distance
/// Returns a value between 0.0 (completely different) and 1.0 (identical)
fn calculate_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    if a_lower == b_lower {
        return 1.0;
    }

    let a_chars: Vec<char> = a_lower.chars().collect();
    let b_chars: Vec<char> = b_lower.chars().collect();

    let a_len = a_chars.len();
    let b_len = b_chars.len();
    let max_len = a_len.max(b_len);

    if max_len == 0 {
        return 1.0;
    }

    // Levenshtein distance via dynamic programming
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0usize; b_len + 1];

    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1) // deletion
                .min(curr[j - 1] + 1) // insertion
                .min(prev[j - 1] + cost); // substitution
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    let distance = prev[b_len];
    1.0 - (distance as f64 / max_len as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_corrections_basic() {
        let corrections = detect_corrections(
            "J'utilise Grok pour la transcription.",
            "J'utilise Groq pour la transcription.",
        );
        assert!(corrections.is_some());
        let c = corrections.unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0], ("Grok".to_string(), "Groq".to_string()));
    }

    #[test]
    fn test_detect_no_change() {
        let corrections = detect_corrections("Hello world", "Hello world");
        assert!(corrections.is_none());
    }

    #[test]
    fn test_detect_multiple_corrections() {
        let corrections = detect_corrections(
            "I met jon and went to parris",
            "I met John and went to Paris",
        );
        assert!(corrections.is_some());
        let c = corrections.unwrap();
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn test_similarity() {
        assert!(calculate_similarity("Grok", "Groq") > 0.5);
        assert!(calculate_similarity("john", "John") == 1.0);
        assert!(calculate_similarity("abc", "xyz") < 0.3);
    }

    // Regression tests for false positives
    #[test]
    fn test_reject_concatenation() {
        // User typed after a word without space: "donne.Magnifique"
        assert!(!is_valid_correction("donne", "donne.Magnifique"));
    }

    #[test]
    fn test_reject_very_different_words() {
        // "marche" → "ma" is not a correction, it's a different word
        assert!(!is_valid_correction("marche", "ma"));
    }

    #[test]
    fn test_reject_short_words() {
        // Single character words should not be corrections
        assert!(!is_valid_correction("a", "I"));
    }

    #[test]
    fn test_accept_real_corrections() {
        assert!(is_valid_correction("Grok", "Groq"));
        assert!(is_valid_correction("parris", "Paris"));
        assert!(is_valid_correction("Kellou", "KELLOU"));
        assert!(is_valid_correction("AmirKs", "AmirKS"));
    }

    #[test]
    fn test_user_continues_typing_after_correction() {
        // User pasted text, corrected one word, then continued typing
        let corrections = detect_corrections(
            "il donne des resultats bien",
            "il donne des résultats bien. Magnifique c'est cool",
        );
        // Should detect "resultats" → "résultats" but ignore the added text
        assert!(corrections.is_some());
        let c = corrections.unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "resultats");
        assert_eq!(c[0].1, "résultats");
    }
}
