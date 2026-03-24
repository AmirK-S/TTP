// TTP - Talk To Paste
// Correction detection - monitors for user corrections after paste
//
// After pasting, waits 10 seconds then reads back the focused text field
// using macOS Accessibility API. Compares with original to detect corrections.

use super::classify::classify_correction;
use super::store::add_entry;
use crate::credentials::get_groq_api_key_internal;
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
                // Get API key for LLM classification gate
                let api_key = match get_groq_api_key_internal(&app_handle) {
                    Ok(Some(key)) => Some(key),
                    _ => None,
                };

                let mut added = false;
                for (original, correction) in &corrections {
                    // LLM validation gate: classify before adding
                    let should_learn = if let Some(ref key) = api_key {
                        match classify_correction(key, original, correction, &current_text).await {
                            Ok(true) => {
                                eprintln!("[Detection] LLM classified '{}' → '{}' as LEARN", original, correction);
                                true
                            }
                            Ok(false) => {
                                eprintln!("[Detection] LLM classified '{}' → '{}' as IGNORE, skipping", original, correction);
                                false
                            }
                            Err(e) => {
                                // Fail closed: do not add if LLM call fails
                                eprintln!("[Detection] LLM classification failed for '{}' → '{}': {}, skipping", original, correction, e);
                                false
                            }
                        }
                    } else {
                        // No API key available — fail closed, skip entry
                        eprintln!("[Detection] No API key available for LLM classification, skipping '{}' → '{}'", original, correction);
                        false
                    };

                    if should_learn {
                        if let Err(e) = add_entry(original, correction) {
                            eprintln!("[Detection] Failed to add dictionary entry: {}", e);
                        } else {
                            added = true;
                        }
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

/// Common stop words that should never be treated as corrections.
/// These are short grammar/context words in French and English that users
/// frequently edit for grammatical reasons, not because of transcription errors.
const STOP_WORDS: &[&str] = &[
    "le", "la", "les", "de", "du", "des", "un", "une", "et", "ou", "en", "au", "aux", "ce", "se",
    "ne", "que", "qui", "the", "a", "an", "is", "are", "was", "were", "be", "to", "of", "in",
    "it", "for", "on", "at",
];

/// Check if a word is a common stop word
fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word.to_lowercase().as_str())
}

/// Check if a word change looks like a real spelling correction vs noise
///
/// A real correction is: "Whysper" → "Whisper", "parris" → "Paris"
/// NOT a correction: "donne" → "donne.Magnifique", "marche" → "ma",
/// "bonjour" → "Bonjour" (case-only), "du" → "de" (stop words)
fn is_valid_correction(orig: &str, curr: &str) -> bool {
    let orig_lower = orig.to_lowercase();
    let curr_lower = curr.to_lowercase();

    // Both words must be at least 3 characters
    // Two-letter words like "du", "de", "je" are grammar words, not transcription errors
    if orig.len() < 3 || curr.len() < 3 {
        return false;
    }

    // Reject case-only differences — the LLM already handles capitalization
    // e.g. "bonjour" → "Bonjour" should not be a dictionary entry
    if orig_lower == curr_lower {
        return false;
    }

    // Reject if either word is a common stop word
    // Stop words are edited for grammar, not transcription correction
    if is_stop_word(orig) || is_stop_word(curr) {
        return false;
    }

    // Reject if one is a proper substring of the other (user concatenated text)
    // e.g. "donne" → "donne.Magnifique" or "test" → "testing"
    if curr_lower.contains(&orig_lower) || orig_lower.contains(&curr_lower) {
        return false;
    }

    // Length ratio check: correction should be similar length (within 2x)
    // Real corrections don't drastically change word length
    let len_ratio = orig.len().max(curr.len()) as f64 / orig.len().min(curr.len()) as f64;
    if len_ratio > 2.0 {
        return false;
    }

    // Reject if both words are short (< 5 chars) and have low similarity (< 0.8)
    // Short common words being swapped are almost always false positives
    let sim = calculate_similarity(orig, curr);
    if orig.len() < 5 && curr.len() < 5 && sim < 0.8 {
        return false;
    }

    // Similarity must be at least 0.65
    // 0.5 was too low and allowed unrelated words like "fait" → "fais"
    if sim < 0.65 {
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
    } else if corrections.len() > 2 {
        // Too many corrections in a single window — the user is likely rewriting
        // text, not correcting transcription errors. Discard all.
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
            "J'utilise Whysper pour la transcription.",
            "J'utilise Whisper pour la transcription.",
        );
        assert!(corrections.is_some());
        let c = corrections.unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0], ("Whysper".to_string(), "Whisper".to_string()));
    }

    #[test]
    fn test_detect_no_change() {
        let corrections = detect_corrections("Hello world", "Hello world");
        assert!(corrections.is_none());
    }

    #[test]
    fn test_detect_two_corrections() {
        // Two corrections is within the limit (max 2)
        let corrections = detect_corrections(
            "I visited Barlin and saw the Colloseum",
            "I visited Berlin and saw the Colosseum",
        );
        assert!(corrections.is_some());
        let c = corrections.unwrap();
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn test_reject_too_many_corrections() {
        // More than 2 corrections means user is rewriting, not correcting
        let corrections = detect_corrections(
            "I visited Barlin and saw Colloseum near Buckingam Palace",
            "I visited Berlin and saw Colosseum near Buckingham Palace",
        );
        // 3 valid corrections: Barlin→Berlin, Colloseum→Colosseum, Buckingam→Buckingham
        // This exceeds the limit of 2, so it should return None
        assert!(corrections.is_none());
    }

    #[test]
    fn test_similarity() {
        assert!(calculate_similarity("Grok", "Groq") > 0.65);
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
        // Two-character words should also be rejected now (min length = 3)
        assert!(!is_valid_correction("du", "de"));
        assert!(!is_valid_correction("je", "le"));
    }

    #[test]
    fn test_reject_case_only_differences() {
        // Case-only changes should NOT be dictionary entries
        // The LLM already handles capitalization
        assert!(!is_valid_correction("bonjour", "Bonjour"));
        assert!(!is_valid_correction("Kellou", "KELLOU"));
        assert!(!is_valid_correction("AmirKs", "AmirKS"));
        assert!(!is_valid_correction("hello", "HELLO"));
    }

    #[test]
    fn test_reject_stop_words() {
        // Stop words should never be treated as corrections
        assert!(!is_valid_correction("les", "des"));
        assert!(!is_valid_correction("the", "teh")); // "the" is a stop word
        assert!(!is_valid_correction("fait", "for")); // "for" is a stop word
        assert!(!is_valid_correction("are", "ore"));
    }

    #[test]
    fn test_reject_short_low_similarity() {
        // Both words < 5 chars with similarity < 0.8 should be rejected
        // These are grammar variations, not transcription errors
        assert!(!is_valid_correction("fait", "fais"));
        assert!(!is_valid_correction("mais", "mois"));
        assert!(!is_valid_correction("Grok", "Groq")); // both 4 chars, sim 0.75 < 0.8
    }

    #[test]
    fn test_accept_real_corrections() {
        assert!(is_valid_correction("parris", "Paris")); // transcription error (6 chars)
        assert!(is_valid_correction("Whysper", "Whisper")); // brand name transcription error
        assert!(is_valid_correction("resultats", "résultats")); // accent correction
        assert!(is_valid_correction("transcription", "transcripcion")); // long word, clear error
        assert!(is_valid_correction("Barlin", "Berlin")); // city name transcription error
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
