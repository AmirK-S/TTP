// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::get_groq_api_key_internal;
use crate::dictionary::detection::start_correction_window;
use crate::dictionary::apply_dictionary;
use crate::history::add_history_entry;
use crate::paste::{check_accessibility, simulate_paste, ClipboardGuard};
// Pill stays visible - no hide needed
use crate::settings::get_settings;
use crate::state::{AppState, RecordingState};
use std::sync::Mutex;
use std::time::Duration;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::time::sleep;

/// Minimum audio file size in bytes (< 10KB is likely too short/silent)
const MIN_AUDIO_SIZE: u64 = 10_000;

/// Maximum audio file size in bytes (25MB Groq API limit)
const MAX_AUDIO_SIZE: u64 = 25_000_000;

/// Common Whisper hallucinations on silent/empty audio
const HALLUCINATIONS: &[&str] = &[
    "thank you",
    "thanks for watching",
    "thanks for listening",
    "bye",
    "goodbye",
    "see you",
    "subscribe",
    "like and subscribe",
    "you",
    "the end",
    ".",
    "",
];

use crate::logging::log_error;
use super::{convert::{convert_to_mono_16khz, convert_to_ogg_opus}, polish_text, transcribe_audio};

/// Progress event sent to frontend during transcription pipeline
#[derive(Clone, serde::Serialize)]
pub struct TranscriptionProgress {
    pub stage: String, // "transcribing", "polishing", "pasting", "complete", "error"
    pub message: String,
}

/// Emit a progress event to the frontend and tag the current pipeline stage in Sentry scope
fn emit_progress(app: &AppHandle, stage: &str, message: &str) {
    // Set Sentry tag for current pipeline stage so errors are attributed correctly
    sentry::configure_scope(|scope| {
        scope.set_tag("pipeline_stage", stage);
    });

    let progress = TranscriptionProgress {
        stage: stage.to_string(),
        message: message.to_string(),
    };
    app.emit("transcription-progress", &progress).ok();
}

/// Show a system notification
fn notify(app: &AppHandle, message: &str) {
    // Try Tauri notification first
    let result = app.notification()
        .builder()
        .title("TTP")
        .body(message)
        .show();

    if result.is_err() {
        // Fallback to osascript
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    "display notification \"{}\" with title \"TTP\"",
                    message.replace("\"", "\\\"")
                ))
                .spawn();
        }
    }
}

/// Set the app state (updates frontend via event)
fn set_state(app: &AppHandle, state: RecordingState) {
    if let Some(app_state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = app_state.try_lock() {
            guard.set_state(state, app);
        }
    }
}

/// Filter out common Whisper hallucinations
fn is_hallucination(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    HALLUCINATIONS
        .iter()
        .any(|h| lower == *h || lower.trim_end_matches('.') == *h)
}

/// Strip LLM wrapper/comparison format from polish output
///
/// When the LLM returns something like:
///   "Voici la version corrigée : texte nettoyé"
///   "Original: ... → Corrected: ..."
/// This function tries to extract just the cleaned text.
fn strip_llm_wrapper(text: &str) -> String {
    let text = text.trim();

    // Pattern: "label: actual text" — take everything after the last colon-prefixed label
    // Look for common prefixes and strip them
    let prefixes = [
        "voici la version corrigée :",
        "voici la version corrigée:",
        "voici le texte corrigé :",
        "voici le texte corrigé:",
        "voici le texte nettoyé :",
        "voici le texte nettoyé:",
        "corrected version:",
        "cleaned version:",
        "cleaned text:",
        "corrected text:",
        "corrected:",
        "here is the cleaned version:",
        "here is the corrected version:",
    ];

    let lower = text.to_lowercase();
    for prefix in &prefixes {
        if let Some(pos) = lower.find(prefix) {
            let after = &text[pos + prefix.len()..].trim();
            if !after.is_empty() {
                // Strip surrounding quotes if present
                let after = after.trim_matches('"').trim_matches('«').trim_matches('»').trim();
                return after.to_string();
            }
        }
    }

    // Pattern: text wrapped in quotes — strip outer quotes
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('«') && text.ends_with('»'))
    {
        return text[1..text.len() - 1].trim().to_string();
    }

    // Fallback: return as-is
    text.to_string()
}

/// Main pipeline function: process a completed recording
///
/// Orchestrates the flow:
/// 1. Transcribe audio via Groq Whisper
/// 2. Polish text via Groq LLM (llama-3.3-70b-versatile)
/// 3. Paste into active app (or clipboard fallback)
///
/// Emits progress events throughout for frontend updates.
pub async fn process_recording(app: &AppHandle, audio_path: String) -> Result<String, String> {
    let pipeline_start = std::time::Instant::now();

    // Set state to Processing
    set_state(app, RecordingState::Processing);

    // Check if audio file exists and has minimum size
    let audio_file = Path::new(&audio_path);
    let file_size = match std::fs::metadata(audio_file) {
        Ok(meta) => meta.len(),
        Err(e) => {
            emit_progress(app, "error", "Audio file not found");
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "api_error", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
            set_state(app, RecordingState::Idle);
            return Err(format!("Audio file error: {}", e));
        }
    };

    if file_size < MIN_AUDIO_SIZE {
        let _ = std::fs::remove_file(&audio_path);
        emit_progress(app, "error", "Recording too short");
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "too_short", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("Recording too short".to_string());
    }

    // AUDI-04: Validate WAV header before any processing
    if let Err(msg) = super::backup::validate_wav(&audio_path) {
        let _ = std::fs::remove_file(&audio_path);
        emit_progress(app, "error", &msg);
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
            "error_category": "corrupt_audio",
            "duration_seconds": pipeline_start.elapsed().as_secs_f64()
        })));
        set_state(app, RecordingState::Idle);
        return Err(msg);
    }

    // Convert stereo 48kHz WAV → mono 16kHz WAV (reduces size ~6x)
    let converted_path = match convert_to_mono_16khz(&audio_path) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("[Pipeline] Conversion failed: {} — sending original", e);
            audio_path.clone()
        }
    };
    let use_converted = converted_path != audio_path;

    // Compress to OGG Opus for smaller upload (graceful degradation: fall back to WAV)
    let ogg_path = match convert_to_ogg_opus(&converted_path) {
        Ok(path) => {
            eprintln!("[Pipeline] OGG Opus encoding succeeded: {}", path);
            Some(path)
        }
        Err(e) => {
            eprintln!("[Pipeline] OGG Opus encoding failed, using WAV: {}", e);
            None
        }
    };

    // Use OGG if available, otherwise the converted WAV
    let final_upload_path = ogg_path.clone().unwrap_or_else(|| converted_path.clone());

    // AUDI-05: If conversion failed and original exceeds 25MB, reject early
    if !use_converted && file_size > MAX_AUDIO_SIZE {
        let original_mb = file_size as f64 / 1_000_000.0;
        let _ = std::fs::remove_file(&audio_path);
        emit_progress(
            app,
            "error",
            &format!("Recording too long ({:.0}MB). Max ~14 min.", original_mb),
        );
        notify(app, "Recording too long -- max ~14 minutes");
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({
            "error_category": "too_long",
            "duration_seconds": pipeline_start.elapsed().as_secs_f64()
        })));
        set_state(app, RecordingState::Idle);
        log_error(&format!("Conversion failed and original too large: {:.1}MB exceeds {}MB limit", original_mb, MAX_AUDIO_SIZE / 1_000_000));
        return Err(format!("Audio too large: {:.1}MB exceeds API limit", original_mb));
    }

    // Check size AFTER conversion (raw WAV can be large but converts down)
    let final_size = std::fs::metadata(&final_upload_path)
        .map(|m| m.len())
        .unwrap_or(file_size);
    let final_mb = final_size as f64 / 1_000_000.0;

    if final_size > MAX_AUDIO_SIZE {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }
        if let Some(ref ogg) = ogg_path { let _ = std::fs::remove_file(ogg); }

        emit_progress(
            app,
            "error",
            &format!("Recording too long ({:.0}MB). Max ~14 min.", final_mb),
        );
        notify(app, "Recording too long — max ~14 minutes");
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "too_long", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        log_error(&format!("Audio too large after conversion: {:.1}MB exceeds {}MB limit", final_mb, MAX_AUDIO_SIZE / 1_000_000));
        return Err(format!("Audio too large: {:.1}MB exceeds API limit", final_mb));
    }

    // Use the converted mono 16kHz WAV for transcription
    let transcription_path = &final_upload_path;

    // Load settings
    let settings = get_settings();

    // Read input mode from app state for analytics
    let input_mode = if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(guard) = state.try_lock() {
            if guard.hands_free_mode { "toggle" } else { "push_to_talk" }
        } else {
            "unknown"
        }
    } else {
        "unknown"
    };

    // Get Groq API key
    let groq_key = get_groq_api_key_internal(app)?.filter(|k| !k.is_empty());

    let api_key = match groq_key {
        Some(key) => key,
        None => {
            let _ = std::fs::remove_file(&audio_path);
            emit_progress(app, "error", "No Groq API key configured");
            if let Some(window) = app.get_webview_window("setup") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            set_state(app, RecordingState::Idle);
            return Err("No Groq API key configured".to_string());
        }
    };

    // AUDI-01: Backup original audio before API call
    let backup_path = match super::backup::backup_audio(app, &audio_path) {
        Ok(path) => Some(path),
        Err(e) => {
            crate::logging::log_warn(&format!("Audio backup failed: {}", e));
            crate::telemetry::analytics::track(app, "backup_failed", Some(serde_json::json!({
                "error": e
            })));
            None // Continue without backup -- don't block transcription
        }
    };

    // Build Whisper prompt from dictionary corrections to bias transcription
    let whisper_prompt = {
        let entries = crate::dictionary::store::get_dictionary();
        if entries.is_empty() {
            None
        } else {
            // Collect unique correction values
            let mut corrections: Vec<String> = entries
                .iter()
                .map(|e| e.correction.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            corrections.sort();

            // Build prompt string, staying under ~200 tokens (~800 chars conservative estimate)
            let prefix = "Glossary: ";
            let mut prompt = prefix.to_string();
            let mut first = true;
            for word in &corrections {
                let addition = if first {
                    word.len()
                } else {
                    2 + word.len() // ", " + word
                };
                if prompt.len() + addition > 800 {
                    break;
                }
                if !first {
                    prompt.push_str(", ");
                }
                prompt.push_str(word);
                first = false;
            }
            if prompt.len() > prefix.len() {
                Some(prompt)
            } else {
                None
            }
        }
    };

    // Stage 1: Transcribe audio via Groq Whisper
    emit_progress(app, "transcribing", "Transcribing...");

    let raw_text = match transcribe_audio(&api_key, transcription_path, whisper_prompt.as_deref()).await {
        Ok(text) => text,
        Err(e) => {
            // AUDI-02: Do NOT delete the original audio on API failure.
            // The backup (and original) remain on disk so the user can inspect or retry.
            log_error(&format!("Transcription failed: {}", e));
            crate::logging::log_info(&format!(
                "Audio preserved in backup after API failure: {}",
                backup_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "no backup".into())
            ));

            // Classify error for analytics
            let error_category = if e.contains("413") || e.to_lowercase().contains("entity too large") || e.to_lowercase().contains("too long") {
                "too_long"
            } else if e.to_lowercase().contains("timeout") || e.to_lowercase().contains("connect") || e.to_lowercase().contains("network") || e.to_lowercase().contains("dns") {
                "network"
            } else {
                "api_error"
            };

            // User-friendly message for 413 / payload too large
            let user_msg = if error_category == "too_long" {
                "Recording too long — try a shorter recording".to_string()
            } else {
                format!("Transcription failed: {}", e)
            };

            emit_progress(app, "error", &user_msg);
            notify(app, "Transcription failed");
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": error_category, "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
            set_state(app, RecordingState::Idle);
            return Err(e);
        }
    };

    // Check for empty transcription (no speech detected)
    if raw_text.trim().is_empty() {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }
        if let Some(ref ogg) = ogg_path { let _ = std::fs::remove_file(ogg); }

        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "No speech detected");
        notify(app, "No speech detected");
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("No speech detected".to_string());
    }

    // Filter out common Whisper hallucinations on silent audio
    if is_hallucination(&raw_text) {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }
        if let Some(ref ogg) = ogg_path { let _ = std::fs::remove_file(ogg); }

        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "No speech detected");
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("No speech detected (filtered)".to_string());
    }

    // Check minimum meaningful content (at least 10 chars after trimming)
    // This prevents the LLM from responding with help messages on near-empty input
    if raw_text.trim().len() < 10 {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }
        if let Some(ref ogg) = ogg_path { let _ = std::fs::remove_file(ogg); }

        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "Recording too short");
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "too_short", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("Recording too short".to_string());
    }

    // Stage 2: Polish text (if enabled)
    let final_text = if settings.ai_polish_enabled {
        emit_progress(app, "polishing", "Processing...");

        match polish_text(&api_key, &raw_text).await {
            Ok(text) => {
                // Detect LLM help responses (happens when input is too minimal)
                let lower = text.to_lowercase();
                let is_llm_help = lower.contains("i'm here to help")
                    || lower.contains("please provide")
                    || lower.contains("i can help")
                    || lower.contains("could you please")
                    || lower.contains("i'm sorry")
                    || lower.contains("i can only process")
                    || lower.contains("feel free to share")
                    || lower.contains("if you have a")
                    || lower.contains("transcription you'd like")
                    || lower.contains("it seems like")
                    || lower.contains("i'd be happy to");

                // Detect LLM showing "original → corrected" comparison format
                let is_comparison = lower.contains("version corrigée")
                    || lower.contains("corrected version")
                    || lower.contains("cleaned version")
                    || lower.contains("here is the")
                    || lower.contains("voici la version")
                    || lower.contains("voici le texte")
                    || lower.contains("original:")
                    || lower.contains("corrected:")
                    || lower.contains("original text")
                    || lower.contains("cleaned text")
                    || (lower.contains("→") && lower.contains("\""));

                // Also suspect if output is much longer than input (LLM adding content)
                let length_ratio = text.len() as f32 / raw_text.len().max(1) as f32;
                let is_too_long = length_ratio > 3.0 && text.len() > 50;

                if is_llm_help || is_too_long {
                    eprintln!("[Pipeline] LLM returned suspicious response, using raw text");
                    raw_text.clone()
                } else if is_comparison {
                    // LLM returned a comparison format — try to extract just the cleaned part
                    // If the response starts with quotes or a label, strip it
                    let cleaned = strip_llm_wrapper(&text);
                    eprintln!("[Pipeline] LLM returned comparison format, extracted: {}", &cleaned[..cleaned.len().min(80)]);
                    cleaned
                } else {
                    text
                }
            }
            Err(e) => {
                eprintln!("[Pipeline] Polish failed, using raw text: {}", e);
                raw_text.clone()
            }
        }
    } else {
        raw_text.clone()
    };

    // Apply dictionary corrections as hard post-processing
    // This guarantees dictionary entries are applied even if the LLM ignored them
    let final_text = apply_dictionary(&final_text);

    // Stage 3: Paste into active app
    emit_progress(app, "pasting", "");

    // Create clipboard guard to save original content
    let clipboard_guard = ClipboardGuard::new(app);

    // ALWAYS write to clipboard first (backup for manual paste)
    if let Err(e) = clipboard_guard.write_text(&final_text) {
        emit_progress(app, "error", "Failed to write to clipboard");
        notify(app, "Failed to copy text to clipboard");
        set_state(app, RecordingState::Idle);
        return Err(e);
    }

    // Check accessibility permission and try to paste
    let has_accessibility = check_accessibility();

    // Use spawn_blocking to run sync paste code safely in async context
    let paste_success = if has_accessibility {
        let paste_result = tokio::task::spawn_blocking(|| {
            std::panic::catch_unwind(|| simulate_paste())
        })
        .await;

        match paste_result {
            Ok(Ok(Ok(()))) => {
                // Wait a bit for paste to complete before restoring clipboard
                sleep(Duration::from_millis(150)).await;

                // Restore original clipboard content
                if let Err(e) = clipboard_guard.restore() {
                    eprintln!("[Pipeline] Failed to restore clipboard: {}", e);
                }

                // Start correction detection window (10 seconds to detect user corrections)
                start_correction_window(app, final_text.clone());

                true
            }
            Ok(Ok(Err(e))) => {
                eprintln!("[Pipeline] Paste simulation failed: {}", e);
                false
            }
            Ok(Err(_)) => {
                eprintln!("[Pipeline] Paste simulation panicked");
                false
            }
            Err(e) => {
                eprintln!("[Pipeline] Paste task failed: {}", e);
                false
            }
        }
    } else {
        eprintln!("[Pipeline] No accessibility permission - using clipboard fallback");

        // Open System Settings to Accessibility pane to help user grant permission
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                .spawn();
        }

        false
    };

    // Save to history (before completing)
    // Store both final and raw text so user can see the unpolished transcription
    let raw_for_history = if settings.ai_polish_enabled {
        Some(raw_text.as_str())
    } else {
        None // No raw text if polish was disabled (they're the same)
    };

    if let Err(e) = add_history_entry(&final_text, raw_for_history) {
        eprintln!("[Pipeline] Failed to save to history: {}", e);
    }

    // Complete with appropriate message
    if paste_success {
        emit_progress(app, "complete", "");
    } else {
        // Clipboard fallback - show error in pill + system notification
        if !has_accessibility {
            emit_progress(app, "error", "Enable Accessibility to auto-paste");
            notify(app, "Add TTP to Accessibility in Settings, then paste with Cmd+V");
        } else {
            emit_progress(app, "error", "Paste failed — Cmd+V to paste");
            notify(app, "Text copied - paste with Cmd+V");
        }
    }

    // Analytics: track successful transcription
    crate::telemetry::analytics::track(app, "transcription_success", Some(serde_json::json!({
        "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
        "word_count": final_text.split_whitespace().count(),
        "polish_enabled": settings.ai_polish_enabled.to_string(),
        "input_mode": input_mode
    })));

    // Clean up audio files after processing
    let _ = std::fs::remove_file(&audio_path);
    if use_converted { let _ = std::fs::remove_file(&converted_path); }
    if let Some(ref ogg) = ogg_path { let _ = std::fs::remove_file(ogg); }

    // AUDI-02: Delete backup only after successful transcription
    if let Some(ref bp) = backup_path {
        super::backup::remove_backup(bp);
    }

    set_state(app, RecordingState::Idle);
    Ok(final_text)
}

/// Tauri command to process a completed recording
///
/// Called by frontend after mic-recorder stops and returns the file path.
/// Runs the full transcription pipeline asynchronously.
#[tauri::command]
pub async fn process_audio(app: AppHandle, audio_path: String) -> Result<String, String> {
    process_recording(&app, audio_path).await
}
