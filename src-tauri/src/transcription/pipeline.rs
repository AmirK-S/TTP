// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::{get_api_key_internal, get_gladia_api_key_internal, get_groq_api_key_internal};
use crate::dictionary::detection::start_correction_window;
use crate::history::add_history_entry;
use crate::paste::{check_accessibility, simulate_paste, ClipboardGuard};
// Pill stays visible - no hide needed
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

use super::ensemble::{transcribe_ensemble, ProviderResult};
use super::fusion::fuse_and_polish;
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
    println!("[Pipeline] Showing notification: {}", message);

    // Try Tauri notification first
    let result = app.notification()
        .builder()
        .title("TTP")
        .body(message)
        .show();

    if result.is_err() {
        println!("[Pipeline] Tauri notification failed, trying osascript");
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

/// Process audio using ensemble mode (parallel transcription + LLM fusion)
///
/// Returns the final text and raw text (for history).
async fn process_ensemble(
    app: &AppHandle,
    audio_path: &str,
    openai_key: Option<String>,
    groq_key: Option<String>,
    gladia_key: Option<String>,
) -> Result<(String, String), String> {
    // Count available providers
    let provider_count = [openai_key.is_some(), groq_key.is_some(), gladia_key.is_some()]
        .iter()
        .filter(|&&x| x)
        .count();

    if provider_count < 2 {
        return Err(format!(
            "Ensemble mode requires at least 2 providers configured (found {})",
            provider_count
        ));
    }

    println!("[Pipeline] Ensemble mode with {} providers", provider_count);
    emit_progress(app, "transcribing", "Transcribing (ensemble)...");

    // Run all providers in parallel
    let results = transcribe_ensemble(
        audio_path,
        openai_key.as_deref(),
        groq_key.as_deref(),
        gladia_key.as_deref(),
    )
    .await?;

    // Check for empty results (all providers failed validation)
    if results.is_empty() {
        return Err("All providers failed or returned empty results".to_string());
    }

    // Build raw text from all provider results (for history)
    let raw_text = results
        .iter()
        .map(|r| format!("[{}]: {}", r.provider, r.text))
        .collect::<Vec<_>>()
        .join("\n\n");

    // If only 1 provider succeeded, fall back to normal polish
    if results.len() == 1 {
        println!("[Pipeline] Only 1 provider succeeded, using normal polish");
        let single_text = results[0].text.clone();

        // Get OpenAI key for polish (required)
        let polish_key = openai_key.ok_or("OpenAI API key required for polish")?;

        emit_progress(app, "polishing", "Processing...");
        let polished = match super::polish_text(&polish_key, &single_text).await {
            Ok(text) => text,
            Err(e) => {
                eprintln!("[Pipeline] Polish failed, using raw text: {}", e);
                single_text
            }
        };

        return Ok((polished, raw_text));
    }

    // Multiple results - fuse with LLM
    emit_progress(app, "polishing", "Fusing transcriptions...");

    // Get OpenAI key for fusion (required)
    let fusion_key = openai_key.ok_or("OpenAI API key required for fusion")?;

    let fused = fuse_and_polish(&fusion_key, &results).await.map_err(|e| {
        eprintln!("[Pipeline] Fusion failed: {}", e);
        // Fall back to first result if fusion fails
        format!("Fusion failed: {}", e)
    })?;

    println!("[Pipeline] Ensemble fusion complete: {} chars", fused.len());
    Ok((fused, raw_text))
}

/// Filter out common Whisper hallucinations
fn is_hallucination(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    HALLUCINATIONS
        .iter()
        .any(|h| lower == *h || lower.trim_end_matches('.') == *h)
}

/// Main pipeline function: process a completed recording
///
/// Orchestrates the flow:
/// 1. Transcribe audio (single provider or ensemble)
/// 2. Polish/fuse text via GPT-4o-mini
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
        println!(
            "[Pipeline] Audio too short: {} bytes < {} minimum",
            file_size, MIN_AUDIO_SIZE
        );
        emit_progress(app, "error", "Recording too short");
        set_state(app, RecordingState::Idle);
        return Err("Recording too short".to_string());
    }

    // Load settings
    let settings = get_settings();

    // Get all available API keys for potential ensemble use
    let openai_key = get_api_key_internal(app)?.filter(|k| !k.is_empty());
    let groq_key = get_groq_api_key_internal(app)?.filter(|k| !k.is_empty());
    let gladia_key = get_gladia_api_key_internal(app)?.filter(|k| !k.is_empty());

    // Check if ensemble mode is enabled and we have enough providers
    let use_ensemble = settings.ensemble_enabled && {
        let count = [openai_key.is_some(), groq_key.is_some(), gladia_key.is_some()]
            .iter()
            .filter(|&&x| x)
            .count();
        count >= 2
    };
    // Process based on mode
    let (final_text, raw_text) = if use_ensemble {
        // Ensemble mode: parallel transcription + LLM fusion
        match process_ensemble(
            app,
            &audio_path,
            openai_key.clone(),
            groq_key.clone(),
            gladia_key.clone(),
        )
        .await
        {
            Ok((final_text, raw_text)) => {
                // Check for hallucinations in fused result
                if is_hallucination(&final_text) {
                    println!("[Pipeline] Filtered hallucination: '{}'", final_text);
                    emit_progress(app, "error", "No speech detected");
                    set_state(app, RecordingState::Idle);
                    return Err("No speech detected (filtered)".to_string());
                }
                (final_text, raw_text)
            }
            Err(e) => {
                emit_progress(app, "error", &format!("Ensemble transcription failed: {}", e));
                notify(app, "Transcription failed");
                set_state(app, RecordingState::Idle);
                return Err(e);
            }
        }
    } else {
        // Single provider mode (existing flow)

        // Get API key based on provider
        let (transcription_key, provider_name) = match settings.transcription_provider {
            TranscriptionProvider::Gladia => (gladia_key.clone(), "Gladia"),
            TranscriptionProvider::Groq => (groq_key.clone(), "Groq"),
            TranscriptionProvider::OpenAI => (openai_key.clone(), "OpenAI"),
        };

        let transcription_api_key = match transcription_key {
            Some(key) => key,
            None => {
                emit_progress(
                    app,
                    "error",
                    &format!("No {} API key configured", provider_name),
                );
                notify(
                    app,
                    &format!("Please set your {} API key in settings", provider_name),
                );
                set_state(app, RecordingState::Idle);
                return Err(format!("No {} API key configured", provider_name));
            }
        };

        // OpenAI key is needed for polish (if enabled)
        if settings.ai_polish_enabled && openai_key.is_none() {
            emit_progress(app, "error", "No OpenAI API key for AI polish");
            notify(app, "Please set your OpenAI API key for AI polish");
            set_state(app, RecordingState::Idle);
            return Err("No OpenAI API key configured for polish".to_string());
        }

        // Stage 1: Transcribe audio
        emit_progress(
            app,
            "transcribing",
            &format!("Transcribing via {}...", provider_name),
        );

        let raw_text = match settings.transcription_provider {
            TranscriptionProvider::Gladia => {
                transcribe_audio_gladia(&transcription_api_key, &audio_path).await
            }
            _ => transcribe_audio(&transcription_api_key, &audio_path).await,
        };

        let raw_text = match raw_text {
            Ok(text) => text,
            Err(e) => {
                eprintln!("[Pipeline] Transcription error: {}", e);
                emit_progress(app, "error", &format!("Transcription failed: {}", e));
                notify(app, "Transcription failed");
                // Don't hide pill - it stays visible
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
        if is_hallucination(&raw_text) {
            println!("[Pipeline] Filtered hallucination: '{}'", raw_text);
            emit_progress(app, "error", "No speech detected");
            set_state(app, RecordingState::Idle);
            return Err("No speech detected (filtered)".to_string());
        }

        // Check minimum meaningful content (at least 10 chars after trimming)
        // This prevents GPT from responding with help messages on near-empty input
        if raw_text.trim().len() < 10 {
            println!("[Pipeline] Transcription too short: '{}' ({} chars)", raw_text, raw_text.trim().len());
            emit_progress(app, "error", "Recording too short");
            set_state(app, RecordingState::Idle);
            return Err("Recording too short".to_string());
        }

        println!("[Pipeline] Raw transcription: {}", raw_text);

        // Stage 2: Polish text (if enabled)
        let polished_text = if settings.ai_polish_enabled {
            emit_progress(app, "polishing", "Processing...");

            match polish_text(openai_key.as_ref().unwrap(), &raw_text).await {
                Ok(text) => {
                    // Detect GPT help responses (happens when input is too minimal)
                    let lower = text.to_lowercase();
                    let is_gpt_help = lower.contains("i'm here to help")
                        || lower.contains("please provide")
                        || lower.contains("i can help")
                        || lower.contains("could you please")
                        || lower.contains("i'm sorry")
                        || lower.contains("i can only process")
                        || lower.contains("feel free to share")
                        || lower.contains("if you have a")
                        || lower.contains("transcription you'd like");

                    // Also suspect if output is much longer than input (GPT adding content)
                    let length_ratio = text.len() as f32 / raw_text.len().max(1) as f32;
                    let is_too_long = length_ratio > 3.0 && text.len() > 50;

                    if is_gpt_help || is_too_long {
                        eprintln!("[Pipeline] GPT returned suspicious response (help={}, ratio={:.1}): '{}', using raw text",
                            is_gpt_help, length_ratio, text);
                        raw_text.clone()
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
            println!("[Pipeline] AI polish disabled, using raw transcription");
            raw_text.clone()
        };

        (polished_text, raw_text)
    };

    println!("[Pipeline] Final text: {}", final_text);

    // Stage 3: Paste into active app
    println!("[Pipeline] Stage 3: Starting paste...");
    emit_progress(app, "pasting", "");

    // Create clipboard guard to save original content
    println!("[Pipeline] Creating clipboard guard...");
    let clipboard_guard = ClipboardGuard::new(app);

    // ALWAYS write to clipboard first (per CONTEXT.md: backup for manual paste)
    println!("[Pipeline] Writing to clipboard...");
    if let Err(e) = clipboard_guard.write_text(&final_text) {
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
    println!("[Pipeline] Paste stage complete, success={}", paste_success);

    // Save to history (before completing)
    // Store both final and raw text for user reference
    // For ensemble mode, raw_text contains all provider results
    // For single mode, raw_text is the unpolished transcription
    let raw_for_history = if settings.ai_polish_enabled || settings.ensemble_enabled {
        Some(raw_text.as_str())
    } else {
        None // No raw text if polish was disabled (they're the same)
    };

    if let Err(e) = add_history_entry(&final_text, raw_for_history) {
        eprintln!("[Pipeline] Failed to save to history: {}", e);
        // Non-critical error, continue with completion
    }

    // Complete with appropriate message
    println!("[Pipeline] Completing...");
    if paste_success {
        println!("[Pipeline] Emitting complete (paste succeeded)");
        emit_progress(app, "complete", "");
    } else {
        // Clipboard fallback - notify user with helpful message
        println!("[Pipeline] Paste failed, showing notification...");
        if !has_accessibility {
            notify(app, "Add TTP to Accessibility in Settings, then paste with Cmd+V");
        } else {
            notify(app, "Text copied - paste with Cmd+V");
        }
        emit_progress(app, "complete", "");
    }

    println!("[Pipeline] Setting state to Idle...");
    // Don't hide pill - it stays visible, just changes appearance
    set_state(app, RecordingState::Idle);
    println!("[Pipeline] Done!");
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
