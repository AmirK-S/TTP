// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::{get_api_key_internal, get_gladia_api_key_internal, get_groq_api_key_internal};
use crate::dictionary::detection::start_correction_window;
use crate::history::add_history_entry;
use crate::paste::{check_accessibility, simulate_paste, ClipboardGuard};
use crate::settings::{get_settings, TranscriptionProvider};
use crate::state::{AppState, RecordingState};
use std::sync::Mutex;
use std::time::Duration;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::time::sleep;

/// Minimum audio file size in bytes (< 10KB is likely too short/silent)
const MIN_AUDIO_SIZE: u64 = 10_000;

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

use super::{polish_text, transcribe_audio, transcribe_audio_gladia};

/// Progress event sent to frontend during transcription pipeline
#[derive(Clone, serde::Serialize)]
pub struct TranscriptionProgress {
    pub stage: String, // "transcribing", "polishing", "pasting", "complete", "error"
    pub message: String,
}

/// Emit a progress event to the frontend
fn emit_progress(app: &AppHandle, stage: &str, message: &str) {
    let progress = TranscriptionProgress {
        stage: stage.to_string(),
        message: message.to_string(),
    };
    app.emit("transcription-progress", &progress).ok();
}

/// Show a system notification
fn notify(app: &AppHandle, message: &str) {
    app.notification()
        .builder()
        .title("TTP")
        .body(message)
        .show()
        .ok();
}

/// Set the app state (updates frontend via event)
fn set_state(app: &AppHandle, state: RecordingState) {
    if let Some(app_state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = app_state.try_lock() {
            guard.set_state(state, app);
        }
    }
}

/// Main pipeline function: process a completed recording
///
/// Orchestrates the flow:
/// 1. Transcribe audio via Whisper API
/// 2. Polish text via GPT-4o-mini
/// 3. Paste into active app (or clipboard fallback)
///
/// Emits progress events throughout for frontend updates.
pub async fn process_recording(app: &AppHandle, audio_path: String) -> Result<String, String> {
    // Set state to Processing
    set_state(app, RecordingState::Processing);

    // Check if audio file exists and has minimum size
    let audio_file = Path::new(&audio_path);
    let file_size = match std::fs::metadata(audio_file) {
        Ok(meta) => meta.len(),
        Err(e) => {
            emit_progress(app, "error", "Audio file not found");
            set_state(app, RecordingState::Idle);
            return Err(format!("Audio file error: {}", e));
        }
    };

    if file_size < MIN_AUDIO_SIZE {
        println!("[Pipeline] Audio too short: {} bytes < {} minimum", file_size, MIN_AUDIO_SIZE);
        emit_progress(app, "error", "Recording too short");
        set_state(app, RecordingState::Idle);
        return Err("Recording too short".to_string());
    }

    // Load settings to check provider
    let settings = get_settings();

    // Get API key based on provider
    let (transcription_key, provider_name) = match settings.transcription_provider {
        TranscriptionProvider::Gladia => {
            let key = get_gladia_api_key_internal(app)?;
            (key, "Gladia")
        }
        TranscriptionProvider::Groq => {
            let key = get_groq_api_key_internal(app)?;
            (key, "Groq")
        }
        TranscriptionProvider::OpenAI => {
            let key = get_api_key_internal(app)?;
            (key, "OpenAI")
        }
    };

    let transcription_api_key = match transcription_key {
        Some(key) => key,
        None => {
            emit_progress(app, "error", &format!("No {} API key configured", provider_name));
            notify(app, &format!("Please set your {} API key in settings", provider_name));
            set_state(app, RecordingState::Idle);
            return Err(format!("No {} API key configured", provider_name));
        }
    };

    // OpenAI key is always needed for polish (GPT-4o-mini)
    let openai_api_key = match get_api_key_internal(app)? {
        Some(key) => key,
        None if settings.ai_polish_enabled => {
            emit_progress(app, "error", "No OpenAI API key for AI polish");
            notify(app, "Please set your OpenAI API key for AI polish");
            set_state(app, RecordingState::Idle);
            return Err("No OpenAI API key configured for polish".to_string());
        }
        None => String::new(), // Polish disabled, no key needed
    };

    // Stage 1: Transcribe audio
    emit_progress(app, "transcribing", &format!("Transcribing via {}...", provider_name));

    let raw_text = match settings.transcription_provider {
        TranscriptionProvider::Gladia => {
            transcribe_audio_gladia(&transcription_api_key, &audio_path).await
        }
        _ => {
            transcribe_audio(&transcription_api_key, &audio_path).await
        }
    };

    let raw_text = match raw_text {
        Ok(text) => text,
        Err(e) => {
            emit_progress(app, "error", &format!("Transcription failed: {}", e));
            notify(app, "Transcription failed");
            set_state(app, RecordingState::Idle);
            return Err(e);
        }
    };

    // Check for empty transcription (no speech detected)
    if raw_text.trim().is_empty() {
        emit_progress(app, "error", "No speech detected");
        notify(app, "No speech detected");
        set_state(app, RecordingState::Idle);
        return Err("No speech detected".to_string());
    }

    // Filter out common Whisper hallucinations on silent audio
    let raw_lower = raw_text.trim().to_lowercase();
    if HALLUCINATIONS.iter().any(|h| raw_lower == *h || raw_lower.trim_end_matches('.') == *h) {
        println!("[Pipeline] Filtered hallucination: '{}'", raw_text);
        emit_progress(app, "error", "No speech detected");
        set_state(app, RecordingState::Idle);
        return Err("No speech detected (filtered)".to_string());
    }

    println!("[Pipeline] Raw transcription: {}", raw_text);

    // Stage 2: Polish text (if enabled)
    let polished_text = if settings.ai_polish_enabled {
        emit_progress(app, "polishing", "Processing...");

        match polish_text(&openai_api_key, &raw_text).await {
            Ok(text) => text,
            Err(e) => {
                // Per CONTEXT.md: Use raw text as fallback if polish fails
                eprintln!("[Pipeline] Polish failed, using raw text: {}", e);
                raw_text.clone()
            }
        }
    } else {
        // AI polish disabled - use raw transcription
        println!("[Pipeline] AI polish disabled, using raw transcription");
        raw_text.clone()
    };

    println!("[Pipeline] Final text: {}", polished_text);

    // Stage 3: Paste into active app
    println!("[Pipeline] Stage 3: Starting paste...");
    emit_progress(app, "pasting", "");

    // Create clipboard guard to save original content
    println!("[Pipeline] Creating clipboard guard...");
    let clipboard_guard = ClipboardGuard::new(app);

    // ALWAYS write to clipboard first (per CONTEXT.md: backup for manual paste)
    println!("[Pipeline] Writing to clipboard...");
    if let Err(e) = clipboard_guard.write_text(&polished_text) {
        emit_progress(app, "error", "Failed to write to clipboard");
        notify(app, "Failed to copy text to clipboard");
        set_state(app, RecordingState::Idle);
        return Err(e);
    }
    println!("[Pipeline] Clipboard write OK");

    // Check accessibility permission and try to paste
    println!("[Pipeline] Checking accessibility...");
    let has_accessibility = check_accessibility();
    println!("[Pipeline] Accessibility: {}", has_accessibility);

    // Use spawn_blocking to run sync paste code safely in async context
    let paste_success = if has_accessibility {
        println!("[Pipeline] Attempting paste simulation...");
        let paste_result = tokio::task::spawn_blocking(|| {
            println!("[Pipeline] In spawn_blocking, calling simulate_paste...");
            std::panic::catch_unwind(|| simulate_paste())
        })
        .await;
        println!("[Pipeline] spawn_blocking returned");

        match paste_result {
            Ok(Ok(Ok(()))) => {
                println!("[Pipeline] Paste OK, waiting 150ms...");
                // Wait a bit for paste to complete before restoring clipboard
                sleep(Duration::from_millis(150)).await;

                println!("[Pipeline] Restoring clipboard...");
                // Restore original clipboard content
                if let Err(e) = clipboard_guard.restore() {
                    eprintln!("[Pipeline] Failed to restore clipboard: {}", e);
                    // Not a critical error - paste succeeded
                }

                // Start correction detection window (10 seconds to detect user corrections)
                start_correction_window(app, polished_text.clone());

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
        false
    };
    println!("[Pipeline] Paste stage complete, success={}", paste_success);

    // Save to history (before completing)
    // Store both polished and raw text for user reference
    let raw_for_history = if settings.ai_polish_enabled {
        Some(raw_text.as_str())
    } else {
        None // No raw text if polish was disabled (they're the same)
    };

    if let Err(e) = add_history_entry(&polished_text, raw_for_history) {
        eprintln!("[Pipeline] Failed to save to history: {}", e);
        // Non-critical error, continue with completion
    }

    // Complete with appropriate message
    println!("[Pipeline] Completing...");
    if paste_success {
        println!("[Pipeline] Emitting complete (paste succeeded)");
        emit_progress(app, "complete", "");
    } else {
        // Clipboard fallback - notify user
        println!("[Pipeline] Paste failed, showing notification...");
        notify(app, "Text copied - paste with Cmd+V");
        emit_progress(app, "complete", "");
    }

    println!("[Pipeline] Setting state to Idle...");
    set_state(app, RecordingState::Idle);
    println!("[Pipeline] Done!");
    Ok(polished_text)
}

/// Tauri command to process a completed recording
///
/// Called by frontend after mic-recorder stops and returns the file path.
/// Runs the full transcription pipeline asynchronously.
#[tauri::command]
pub async fn process_audio(app: AppHandle, audio_path: String) -> Result<String, String> {
    process_recording(&app, audio_path).await
}
