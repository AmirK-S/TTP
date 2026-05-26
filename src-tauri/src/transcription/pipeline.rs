// TTP - Talk To Paste
// Pipeline orchestration - coordinates transcribe -> polish -> paste flow
//
// This module ties together the recording completion with transcription,
// text polishing, and auto-paste functionality.

use crate::credentials::get_groq_api_key_internal;
use crate::dictionary::detection::start_correction_window;
use crate::dictionary::apply_dictionary;
use crate::history::add_history_entry;
use crate::paste::{check_accessibility, simulate_paste, simulate_typing, ClipboardGuard};
#[cfg(target_os = "macos")]
use crate::paste::{probe_accessibility, reset_accessibility_tcc};
// Pill stays visible - no hide needed
use crate::settings::get_settings;
use crate::state::{AppState, RecordingState};
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::time::sleep;

/// Global rate limiter for `process_audio`: 20 transcriptions per 60s (sliding window via GCRA).
/// Sized to never trip a power user transcribing rapidly while still capping a runaway
/// frontend loop in the seconds it would take to notice.
static PROCESS_AUDIO_LIMITER: OnceLock<DefaultDirectRateLimiter> = OnceLock::new();

/// Maximum audio file size in bytes (25MB Groq API limit)
const MAX_AUDIO_SIZE: u64 = 25_000_000;

/// Threshold above which we fall back to clipboard+Cmd+V instead of direct
/// keystroke injection. Below this, we type the transcription directly via
/// enigo (CGEventKeyboardSetUnicodeString on macOS) — the clipboard is never
/// touched, eliminating the NSPasteboard read/restore race that bites slow
/// Electron targets (Slack, Notion, Mail). 99 % of voice transcriptions are
/// well under this length; over it, character-by-character typing would feel
/// laggy so we accept the clipboard race for the rare long-text case and
/// mitigate it with a longer post-paste wait (1500 ms).
const DIRECT_TYPING_MAX_CHARS: usize = 2000;

/// How long to wait after Cmd+V before restoring the user's pre-record
/// clipboard, when we have to use the clipboard path (text > DIRECT_TYPING_MAX_CHARS).
/// Slow Electron apps (Slack, Notion, Mail, Discord) can take 200–600 ms to
/// actually read NSPasteboard after receiving Cmd+V — restoring sooner makes
/// the target read the OLD clipboard. 1500 ms is a conservative ceiling that
/// covers the worst observed Electron pauses without making the UX painful
/// (the user only hits this branch for >2000-char transcriptions, which are
/// rare and themselves take seconds to produce).
const CLIPBOARD_PASTE_RESTORE_DELAY_MS: u64 = 1500;

/// Common Whisper hallucinations on silent/empty audio
const HALLUCINATIONS: &[&str] = &[
    // English
    "thank you",
    "thanks for watching",
    "thank you for watching",
    "thanks for listening",
    "thanks for watching please subscribe",
    "bye",
    "goodbye",
    "see you",
    "subscribe",
    "like and subscribe",
    "you",
    "the end",
    "so",
    "the",
    "oh",
    "okay",
    "uh",
    "i'm sorry",
    "hello everyone welcome to my channel",
    // French
    "merci d'avoir regarde cette video",
    "je vous remercie",
    // Subtitle attribution hallucinations (all languages)
    "subtitles by the amara org community",
    "sous-titres realises par la communaute d'amara.org",
    "sous-titrage st' 501",
    "transcription by castingwords",
    ".",
    "",
];

/// Substrings that indicate a hallucination (partial match)
const HALLUCINATION_SUBSTRINGS: &[&str] = &[
    "amara.org",
    "sous-titr",
    "subtitles by",
    "transcription by",
    "soustitreur.com",
    "www.mooji.org",
];

use crate::logging::log_error;
use super::cleanup::cleanup;
use super::polish::{guard_polish, GuardVerdict};
use super::{convert::convert_to_mono_16khz, polish_text, transcribe_audio};

/// Progress event sent to frontend during transcription pipeline.
///
/// `message` carries a translation KEY (e.g. `"error.no_speech"`,
/// `"progress.transcribing"`) — not a finished English string. The frontend
/// resolves it via i18next using its own `t()` function, so the language of
/// the pill text matches whatever the user selected in settings.
///
/// Empty `message` means "no text" — the frontend hides the message label
/// in that case (used for stages like `"pasting"` and `"complete"` where
/// the stage icon is enough).
///
/// `params` carries interpolation values for `{{var}}` placeholders inside
/// the translation (e.g. `{"mb": "42"}` for `error.audio_too_large`).
/// Serialised as a JSON object on the wire; `None` means no interpolation
/// needed.
#[derive(Clone, serde::Serialize)]
pub struct TranscriptionProgress {
    pub stage: String, // "transcribing", "polishing", "pasting", "complete", "error"
    /// Translation key for the frontend to resolve via i18next (e.g. "error.no_speech").
    /// Empty string means "no message" (the frontend hides the text).
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Emit a progress event to the frontend and tag the current pipeline stage in Sentry scope.
///
/// `message` MUST be a translation key from `en.json` / `fr.json` (or the
/// empty string when no message is appropriate). Do not pass raw English
/// here — the frontend will surface it as a missing-key tag.
fn emit_progress(app: &AppHandle, stage: &str, message: &str, params: Option<serde_json::Value>) {
    // Set Sentry tag for current pipeline stage so errors are attributed correctly
    sentry::configure_scope(|scope| {
        scope.set_tag("pipeline_stage", stage);
    });

    let progress = TranscriptionProgress {
        stage: stage.to_string(),
        message: message.to_string(),
        params,
    };
    app.emit("transcription-progress", &progress).ok();
}

/// Show a system notification. `message` is the already-translated body
/// (this function does not look up keys — callers pass `crate::i18n::tr(...)`).
fn notify(app: &AppHandle, message: &str) {
    // Try Tauri notification first
    let result = app.notification()
        .builder()
        .title(crate::i18n::tr("notification.appName"))
        .body(message)
        .show();

    if result.is_err() {
        // Fallback to osascript
        #[cfg(target_os = "macos")]
        {
            let app_name = crate::i18n::tr("notification.appName");
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    "display notification \"{}\" with title \"{}\"",
                    message.replace("\"", "\\\""),
                    app_name.replace("\"", "\\\"")
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
    // Exact match (with/without trailing period)
    let exact = HALLUCINATIONS
        .iter()
        .any(|h| lower == *h || lower.trim_end_matches('.') == *h);
    if exact {
        return true;
    }
    // Substring match for common attribution/credit hallucinations
    HALLUCINATION_SUBSTRINGS
        .iter()
        .any(|sub| lower.contains(sub))
}

// strip_llm_wrapper() removed in Polaris v3.0.0: the new anti-injection
// polish prompt requests structured JSON {intent, polished} and serde does
// the unwrapping. No more prefix-matching hacks — if the LLM emits anything
// other than valid JSON, the polish.rs fallback treats it as raw polished
// text and the post-LLM guards catch real anomalies (length explosion,
// low overlap, refusal markers).

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

    // Check if audio file exists
    let audio_file = Path::new(&audio_path);
    let file_size = match std::fs::metadata(audio_file) {
        Ok(meta) => meta.len(),
        Err(e) => {
            emit_progress(app, "error", "error.audio_file_not_found", None);
            crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "api_error", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
            set_state(app, RecordingState::Idle);
            return Err(format!("Audio file error: {}", e));
        }
    };

    // AUDI-04: Validate WAV header before any processing.
    // The validation error detail is preserved in the developer log; the
    // user-facing message is a single localized "audio corrupt" key.
    if let Err(msg) = super::backup::validate_wav(&audio_path) {
        let _ = std::fs::remove_file(&audio_path);
        log_error(&format!("WAV validation failed: {}", msg));
        emit_progress(app, "error", "error.audio_corrupt", None);
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
            // Tell the user the next stage may reject it (~25 MB cap on
            // un-compressed audio is hit much earlier than on compressed).
            emit_progress(app, "transcribing", "error.compression_failed", None);
            audio_path.clone()
        }
    };
    let use_converted = converted_path != audio_path;

    let final_upload_path = converted_path.clone();

    // AUDI-05: If conversion failed and original exceeds 25MB, reject early
    if !use_converted && file_size > MAX_AUDIO_SIZE {
        let original_mb = file_size as f64 / 1_000_000.0;
        let _ = std::fs::remove_file(&audio_path);
        let mb_str = format!("{:.0}", original_mb);
        emit_progress(
            app,
            "error",
            "error.audio_too_large",
            Some(serde_json::json!({ "mb": mb_str })),
        );
        notify(app, &crate::i18n::tr("notification.recordingTooLong"));
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


        let mb_str = format!("{:.0}", final_mb);
        emit_progress(
            app,
            "error",
            "error.audio_too_large",
            Some(serde_json::json!({ "mb": mb_str })),
        );
        notify(app, &crate::i18n::tr("notification.recordingTooLong"));
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
            if use_converted { let _ = std::fs::remove_file(&converted_path); }
            emit_progress(app, "error", "error.no_api_key", None);
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
            // Local log keeps the full error (incl. paths) for the user's
            // own debugging; analytics gets only the error category to
            // avoid leaking filesystem paths off-device.
            crate::logging::log_warn(&format!("Audio backup failed: {}", e));
            let category = if e.contains("Permission") { "permission_denied" }
                else if e.contains("space") || e.contains("No space") { "disk_full" }
                else if e.contains("dir") { "create_dir_failed" }
                else { "other" };
            crate::telemetry::analytics::track(app, "backup_failed", Some(serde_json::json!({
                "category": category
            })));
            None // Continue without backup -- don't block transcription
        }
    };

    // Build Whisper prompt from dictionary corrections to bias transcription
    let whisper_prompt = {
        // Bilingual hint helps Whisper narrow language detection to FR/EN
        let mut prompt = "French and English bilingual speaker.".to_string();

        let entries = crate::dictionary::store::get_dictionary();
        if !entries.is_empty() {
            // Collect unique correction values
            let mut corrections: Vec<String> = entries
                .iter()
                .map(|e| e.correction.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            corrections.sort();

            // Append glossary, staying under ~200 tokens (~800 chars conservative estimate)
            prompt.push_str(" Glossary: ");
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
        }

        Some(prompt)
    };

    // Stage 1: Transcribe audio via Groq Whisper
    emit_progress(app, "transcribing", "progress.transcribing", None);

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

            // Classify error for analytics — capture HTTP status alongside the category
            // so Sentry/Aptabase can split the catch-all "api_error" bucket by code.
            let (error_category, status_code): (&str, Option<u16>) = if e.contains(" 429 ") || e.contains(": 429") {
                ("rate_limited", Some(429))
            } else if e.contains(" 401 ") || e.contains(": 401") {
                ("invalid_api_key", Some(401))
            } else if e.contains(" 403 ") || e.contains(": 403") {
                ("invalid_api_key", Some(403))
            } else if e.contains("413") {
                ("too_long", Some(413))
            } else if e.to_lowercase().contains("entity too large") || e.to_lowercase().contains("too long") {
                ("too_long", None)
            } else if e.to_lowercase().contains("timeout") || e.to_lowercase().contains("connect") || e.to_lowercase().contains("network") || e.to_lowercase().contains("dns") {
                ("network", None)
            } else {
                ("api_error", None)
            };

            // User-facing translation key + optional interpolation params,
            // based on error category. The frontend resolves the key into
            // localized pill text via i18next.
            let (user_key, user_params) = match error_category {
                "rate_limited" => ("error.transcription_rate_limited", None),
                "invalid_api_key" => ("error.transcription_invalid_key", None),
                "too_long" => ("error.transcription_too_long", None),
                _ => (
                    "error.transcription_generic",
                    Some(serde_json::json!({ "error": e.clone() })),
                ),
            };

            emit_progress(app, "error", user_key, user_params);
            notify(app, &crate::i18n::tr("notification.transcriptionFailed"));
            let mut payload = serde_json::json!({
                "error_category": error_category,
                "duration_seconds": pipeline_start.elapsed().as_secs_f64(),
            });
            if let Some(code) = status_code {
                payload["status_code"] = code.into();
            }
            crate::telemetry::analytics::track(app, "transcription_failed", Some(payload));
            set_state(app, RecordingState::Idle);
            return Err(e);
        }
    };

    // Check for empty transcription (no speech detected)
    if raw_text.trim().is_empty() {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }


        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "error.no_speech", None);
        notify(app, &crate::i18n::tr("notification.noSpeech"));
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("No speech detected".to_string());
    }

    // Filter out dictionary-induced hallucinations: if the entire transcription
    // is just 1-2 words that all appear in the dictionary, Whisper likely
    // hallucinated a glossary word on silence rather than transcribing real speech.
    {
        let dict_entries = crate::dictionary::store::get_dictionary();
        if !dict_entries.is_empty() {
            let words: Vec<&str> = raw_text.trim().split_whitespace().collect();
            if words.len() <= 2 {
                let dict_words: Vec<String> = dict_entries.iter()
                    .map(|e| e.correction.to_lowercase())
                    .collect();
                let all_in_dict = words.iter().all(|w| {
                    let w_lower = w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                    dict_words.iter().any(|d| d == &w_lower)
                });
                if all_in_dict {
                    let _ = std::fs::remove_file(&audio_path);
                    if use_converted { let _ = std::fs::remove_file(&converted_path); }
                    if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
                    emit_progress(app, "error", "error.no_speech", None);
                    crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
                    set_state(app, RecordingState::Idle);
                    return Err("No speech detected (glossary ghost)".to_string());
                }
            }
        }
    }

    // Filter out common Whisper hallucinations on silent audio
    if is_hallucination(&raw_text) {
        let _ = std::fs::remove_file(&audio_path);
        if use_converted { let _ = std::fs::remove_file(&converted_path); }


        if let Some(ref bp) = backup_path { super::backup::remove_backup(bp); }
        emit_progress(app, "error", "error.no_speech", None);
        crate::telemetry::analytics::track(app, "transcription_failed", Some(serde_json::json!({"error_category": "no_speech", "duration_seconds": pipeline_start.elapsed().as_secs_f64()})));
        set_state(app, RecordingState::Idle);
        return Err("No speech detected (filtered)".to_string());
    }

    // Phase-1 deterministic cleanup BEFORE the LLM. Handles punctuation
    // commands, tech term normalization, acronyms, repetitions, standalone
    // fillers. Idempotent. Roughly 80% of cleanups happen here without
    // touching the LLM — faster, cheaper, safer.
    let cleaned_text = cleanup(&raw_text);

    // Stage 2: Polish text (if enabled AND user has quota)
    let polish_quota_ok = if !settings.ai_polish_enabled {
        false
    } else if crate::licensing::is_pro_or_trial_disk() {
        true
    } else {
        let used = crate::usage::polish_count_this_month(&crate::usage::load_usage());
        if used < crate::licensing::FREE_POLISH_PER_MONTH {
            true
        } else {
            let used_str = used.to_string();
            let limit_str = crate::licensing::FREE_POLISH_PER_MONTH.to_string();
            // Once-per-month gate (see Windows toast spam fix v2.1.7).
            if crate::usage::should_notify_polish_cap_once_this_month() {
                notify(
                    app,
                    &crate::i18n::tr_with(
                        "notification.polishLimitReached",
                        &[("used", &used_str), ("limit", &limit_str)],
                    ),
                );
            }
            crate::telemetry::analytics::track(
                app,
                "free_cap_hit",
                Some(serde_json::json!({"feature": "ai_polish"})),
            );
            false
        }
    };

    let final_text = if polish_quota_ok {
        emit_progress(app, "polishing", "progress.polishing", None);

        let polish_start = std::time::Instant::now();
        match polish_text(&api_key, &cleaned_text).await {
            Ok(result) => {
                crate::usage::record_polish_success();
                let polish_ms = polish_start.elapsed().as_millis() as u64;

                // Post-LLM guards: length ratio, content-word Jaccard,
                // refusal markers. On rejection → fall back to phase-1
                // cleaned text + log to Sentry. Silent fallback (no UI
                // popup mid-dictation, just slightly less-polished text).
                match guard_polish(&cleaned_text, &result.polished) {
                    GuardVerdict::Accept => {
                        let mut data = std::collections::BTreeMap::new();
                        data.insert("intent".to_string(), format!("{:?}", result.intent).into());
                        data.insert("latency_ms".to_string(), polish_ms.into());
                        sentry::add_breadcrumb(sentry::Breadcrumb {
                            ty: "polish".into(),
                            category: Some("polish.success".into()),
                            level: sentry::Level::Info,
                            data,
                            ..Default::default()
                        });
                        // Intent classification is captured in the Sentry
                        // breadcrumb above for future per-app routing (v3.1+).
                        result.polished
                    }
                    GuardVerdict::Reject(reason) => {
                        log_error(&format!(
                            "Polish guard rejected: {} (intent={:?}, polish_ms={})",
                            reason, result.intent, polish_ms
                        ));
                        let mut data = std::collections::BTreeMap::new();
                        data.insert("reason".to_string(), reason.into());
                        data.insert("intent".to_string(), format!("{:?}", result.intent).into());
                        data.insert("latency_ms".to_string(), polish_ms.into());
                        sentry::add_breadcrumb(sentry::Breadcrumb {
                            ty: "polish".into(),
                            category: Some("polish.guard_reject".into()),
                            level: sentry::Level::Warning,
                            data,
                            ..Default::default()
                        });
                        crate::telemetry::analytics::track(
                            app,
                            "polish_guard_rejected",
                            Some(serde_json::json!({ "reason": reason })),
                        );
                        cleaned_text.clone()
                    }
                }
            }
            Err(e) => {
                eprintln!("[Pipeline] Polish failed, using cleaned text: {}", e);
                let category = if e.contains(" 429 ") || e.contains(": 429") {
                    "rate_limited"
                } else if e.contains(" 401 ") || e.contains(": 401") || e.contains(" 403 ") || e.contains(": 403") {
                    "invalid_api_key"
                } else {
                    "polish_failed"
                };
                if category == "polish_failed" {
                    emit_progress(app, "pasting", "error.polish_unavailable", None);
                }
                crate::telemetry::analytics::track(app, "polish_failed", Some(serde_json::json!({
                    "category": category
                })));
                cleaned_text.clone()
            }
        }
    } else {
        cleaned_text.clone()
    };

    // Apply dictionary corrections as hard post-processing.
    // Even with the new polish pipeline, the personal dictionary is a hard
    // guarantee — applied after polish (and after cleanup) so user-specific
    // term spellings always win.
    let final_text = apply_dictionary(&final_text);

    // Stage 3: Paste into active app
    emit_progress(app, "pasting", "", None);

    // Create clipboard guard to save original content.
    // We always write the transcription to the clipboard first so that if AX
    // is denied — or anything in the paste path fails — the user can still
    // recover the text with a manual Cmd+V. The transcription stays in the
    // clipboard until either (a) we successfully paste/type and restore, or
    // (b) we error out (in which case we deliberately leave it).
    let clipboard_guard = ClipboardGuard::new(app);

    if let Err(e) = clipboard_guard.write_text(&final_text) {
        emit_progress(app, "error", "error.clipboard_write_failed", None);
        notify(app, &crate::i18n::tr("notification.clipboardFailed"));
        set_state(app, RecordingState::Idle);
        return Err(e);
    }

    // Check accessibility permission and try to paste.
    //
    // On macOS we use `probe_accessibility()` — a real AXUIElement API call —
    // instead of just `check_accessibility()` (which only reads the TCC grant
    // flag). After an in-place app-bundle replacement (auto-updater), macOS
    // commonly leaves the TCC entry marked "trusted" while the actual AX API
    // is silently rejected. The cheap check then lies: pipeline thinks paste
    // will work, simulate_typing/paste calls CGEventPost which returns void
    // (no error), and the user sees clipboard + history populated but no
    // keystroke ever delivered. The probe catches this stale state.
    //
    // If we detect the stale state, also call `reset_accessibility_tcc()` so
    // the entry gets wiped — the user's next interaction will re-prompt
    // cleanly via the System Settings deep link below instead of looking at
    // a "TTP is enabled but not really" entry.
    #[cfg(target_os = "macos")]
    let has_accessibility = {
        let trusted_flag = check_accessibility();
        let actually_works = probe_accessibility();
        if trusted_flag && !actually_works {
            eprintln!("[Pipeline] Accessibility TCC entry is stale — resetting so user can re-grant.");
            if let Err(e) = reset_accessibility_tcc() {
                eprintln!("[Pipeline] reset_accessibility_tcc failed: {}", e);
            }
        }
        actually_works
    };
    #[cfg(not(target_os = "macos"))]
    let has_accessibility = check_accessibility();

    // Pick the paste strategy based on length.
    //
    // For short text (the overwhelming majority of voice transcriptions) we
    // type the characters directly via enigo. The clipboard is written for
    // manual-Cmd+V fallback only — the actual insertion never touches the
    // pasteboard, so there is no race: as soon as enigo.text() returns, every
    // keystroke has been delivered to the focused app's HID queue and we can
    // restore the user's pre-record clipboard immediately.
    //
    // For long text we keep the clipboard+Cmd+V path because typing thousands
    // of characters one by one would be unacceptably slow. We trade the race
    // for speed and mitigate by waiting CLIPBOARD_PASTE_RESTORE_DELAY_MS
    // (1500 ms) before restoring — long enough for slow Electron apps to
    // actually read the pasteboard after Cmd+V.
    let use_direct_typing = final_text.chars().count() <= DIRECT_TYPING_MAX_CHARS;

    // Use spawn_blocking to run sync paste code safely in async context.
    // We deliberately use tauri::async_runtime::spawn_blocking instead of the
    // raw tokio variant because it binds the worker to the same runtime Tauri
    // is using — bare tokio::task::spawn_blocking has been seen to panic with
    // "there is no reactor running" on macOS when the call site is somehow
    // detached from the active runtime (see sounds.rs / audio_monitor.rs
    // for the same fix pattern).
    let paste_success = if has_accessibility {
        let paste_result = if use_direct_typing {
            let text_for_typing = final_text.clone();
            tauri::async_runtime::spawn_blocking(move || {
                std::panic::catch_unwind(|| simulate_typing(&text_for_typing))
            })
            .await
        } else {
            tauri::async_runtime::spawn_blocking(|| {
                std::panic::catch_unwind(|| simulate_paste())
            })
            .await
        };

        match paste_result {
            Ok(Ok(Ok(()))) => {
                if use_direct_typing {
                    // Direct typing already delivered every character to the
                    // focused app — no async pasteboard read in flight, so we
                    // can restore the user's clipboard immediately.
                } else {
                    // Clipboard+Cmd+V: give the target app a generous window
                    // to actually read NSPasteboard before we overwrite it
                    // with the original contents.
                    sleep(Duration::from_millis(CLIPBOARD_PASTE_RESTORE_DELAY_MS)).await;
                }

                // Restore original clipboard content
                if let Err(e) = clipboard_guard.restore() {
                    eprintln!("[Pipeline] Failed to restore clipboard: {}", e);
                }

                // Start correction detection window (10 seconds to detect user corrections).
                // Skip when the final text is empty — the detection task would otherwise
                // poll Accessibility API for 15s for no reason (phantom F5 race, empty API).
                if final_text.trim().is_empty() {
                    eprintln!("[Pipeline] start_correction_window skipped — empty final_text");
                } else {
                    start_correction_window(app, final_text.clone());
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

        // Open System Settings to Accessibility pane to help user grant permission
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                .spawn();
        }

        false
    };

    // Save to history (before completing) — only when user has history enabled
    if settings.history_enabled {
        // Store both final and raw text so user can see the unpolished transcription
        let raw_for_history = if settings.ai_polish_enabled {
            Some(raw_text.as_str())
        } else {
            None // No raw text if polish was disabled (they're the same)
        };

        if let Err(e) = add_history_entry(&final_text, raw_for_history) {
            eprintln!("[Pipeline] Failed to save to history: {}", e);
        }
    }

    // Complete with appropriate message
    if paste_success {
        emit_progress(app, "complete", "", None);
    } else {
        // Clipboard fallback - show error in pill + system notification
        if !has_accessibility {
            emit_progress(app, "error", "error.enable_accessibility", None);
            notify(app, &crate::i18n::tr("notification.addToAccessibility"));
        } else {
            emit_progress(app, "error", "error.paste_failed", None);
            notify(app, &crate::i18n::tr("notification.textCopied"));
        }
    }

    // Record the successful transcription in the local daily-stats bucket
    // so the in-app Analytics section can show "this week / this month".
    // Local-only — no network. Capped at u32 to keep the on-disk payload
    // bounded; even a power user shouldn't dent that ceiling per day.
    let word_count = final_text.split_whitespace().count();
    let char_count = final_text.chars().count();
    crate::usage::record_transcription(
        word_count.try_into().unwrap_or(u32::MAX),
        char_count.try_into().unwrap_or(u32::MAX),
    );

    // Clean up audio files after processing
    let _ = std::fs::remove_file(&audio_path);
    if use_converted { let _ = std::fs::remove_file(&converted_path); }

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
    let limiter = PROCESS_AUDIO_LIMITER.get_or_init(|| {
        RateLimiter::direct(Quota::per_minute(NonZeroU32::new(20).unwrap()))
    });
    if limiter.check().is_err() {
        // Return the translation key rather than a finished string — the
        // frontend resolves it via i18next so the toast/pill matches the
        // user's selected language.
        return Err("error.rate_limit_exceeded".to_string());
    }
    process_recording(&app, audio_path).await
}
