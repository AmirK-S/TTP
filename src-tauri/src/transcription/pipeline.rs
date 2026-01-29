// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::get_api_key_internal;
use crate::paste::{check_accessibility, simulate_paste, ClipboardGuard};
use crate::state::{AppState, RecordingState};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::time::sleep;

use super::{polish_text, transcribe_audio};

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

    // Get API key from credentials
    let api_key = match get_api_key_internal(app)? {
        Some(key) => key,
        None => {
            emit_progress(app, "error", "No API key configured");
            notify(app, "Please set your OpenAI API key in settings");
            set_state(app, RecordingState::Idle);
            return Err("No API key configured".to_string());
        }
    };

    // Stage 1: Transcribe audio
    emit_progress(app, "transcribing", "Transcribing...");

    let raw_text = match transcribe_audio(&api_key, &audio_path).await {
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

    println!("[Pipeline] Raw transcription: {}", raw_text);

    // Stage 2: Polish text
    emit_progress(app, "polishing", "Processing...");

    let polished_text = match polish_text(&api_key, &raw_text).await {
        Ok(text) => text,
        Err(e) => {
            // Per CONTEXT.md: Use raw text as fallback if polish fails
            eprintln!("[Pipeline] Polish failed, using raw text: {}", e);
            raw_text.clone()
        }
    };

    println!("[Pipeline] Polished text: {}", polished_text);

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
