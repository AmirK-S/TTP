// TTP - Talk To Paste
// Global keyboard shortcut handling with push-to-talk and double-tap toggle
//
// Hands-free semantics:
//   * Settings toggle `hands_free_mode` = persistent preference. When true,
//     every single press is a toggle (press to start, press to stop).
//   * Double-tap = TRANSIENT override for one recording. Sets the AppState
//     `session_hands_free` override only — never touches the persisted
//     setting. The override is cleared automatically when set_state
//     transitions back to Idle (state.rs), so there is no "restore on
//     stop" code in this file: it would be redundant and a future refactor
//     hazard.

use crate::settings::get_settings;
use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use crate::tray::{set_recording_icon, should_show_pill, show_pill, hide_pill};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Double-tap detection threshold in milliseconds
const DOUBLE_TAP_THRESHOLD_MS: u128 = 300;

/// Set up global keyboard shortcuts for recording control
pub fn setup_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings = get_settings();
    let shortcut_str = settings.shortcut;

    // "FnKey" is handled by the fnkey module, not global shortcuts
    if shortcut_str == "FnKey" {
        return Ok(());
    }

    let shortcut = shortcut_str.parse::<Shortcut>().unwrap_or_else(|_| {
        #[cfg(target_os = "macos")]
        { "Alt+Space".parse::<Shortcut>().expect("hardcoded fallback shortcut must parse") }
        #[cfg(not(target_os = "macos"))]
        { "Ctrl+Space".parse::<Shortcut>().expect("hardcoded fallback shortcut must parse") }
    });

    // Use register() instead of on_shortcut() - handler is set in Builder
    app.global_shortcut().register(shortcut)?;
    Ok(())
}

/// Update the global shortcut at runtime
pub fn update_shortcut(app: &AppHandle, new_shortcut: &str) -> Result<(), String> {
    let shortcut = new_shortcut.parse::<Shortcut>()
        .map_err(|e| format!("Invalid shortcut '{}': {}", new_shortcut, e))?;

    let global_shortcut = app.global_shortcut();
    global_shortcut.unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;

    // Use register() - handler is set in Builder
    global_shortcut.register(shortcut)
        .map_err(|e| format!("Failed to register '{}': {}", new_shortcut, e))?;
    Ok(())
}

/// Handle shortcut event - dispatches to press/release handlers (public for Builder handler)
pub fn handle_shortcut_event_public(app: &AppHandle, shortcut_state: ShortcutState) {
    let state = app.state::<Mutex<AppState>>();

    let Ok(mut app_state) = state.try_lock() else {
        // Lock contended — a previous press/release handler is still running.
        // Surfacing this in Sentry so we can quantify how often hotkey events
        // get dropped under spam (currently silent → users blame the hotkey).
        sentry::add_breadcrumb(sentry::Breadcrumb {
            category: Some("shortcuts".to_string()),
            message: Some("try_lock contended at handle_shortcut_event_public".to_string()),
            level: sentry::Level::Warning,
            ..Default::default()
        });
        crate::logging::log_warn("[Shortcuts] try_lock contended at handle_shortcut_event_public");
        crate::trace::event(
            "hotkey.event_dropped",
            serde_json::json!({ "at": "handle_shortcut_event_public" }),
        );
        return;
    };

    match shortcut_state {
        ShortcutState::Pressed => handle_shortcut_pressed(&mut app_state, app),
        ShortcutState::Released => handle_shortcut_released(&mut app_state, app),
    }
}

/// Handle FN key double-tap event - toggles hands-free mode
pub fn handle_fn_double_tap(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();

    let Ok(mut app_state) = state.try_lock() else {
        // Same rationale as handle_shortcut_event_public: track dropped events.
        sentry::add_breadcrumb(sentry::Breadcrumb {
            category: Some("shortcuts".to_string()),
            message: Some("try_lock contended at handle_fn_double_tap".to_string()),
            level: sentry::Level::Warning,
            ..Default::default()
        });
        crate::logging::log_warn("[Shortcuts] try_lock contended at handle_fn_double_tap");
        crate::trace::event(
            "hotkey.event_dropped",
            serde_json::json!({ "at": "handle_fn_double_tap" }),
        );
        return;
    };

    match app_state.recording_state {
        RecordingState::Idle => {
            // Transient: enter hands-free for this recording only via the
            // session override. The persisted setting (mirrored into
            // `hands_free_mode`) is untouched, and the override clears
            // automatically when the session transitions to Idle.
            app_state.enter_hands_free_session();
            start_recording(&mut app_state, app);
        }
        RecordingState::Recording if app_state.effective_hands_free() => {
            stop_recording(&mut app_state, app);
            // No explicit restore needed: `session_hands_free` is cleared
            // inside set_state on Idle, so the next read of
            // effective_hands_free returns the persisted setting.
        }
        _ => {}
    }
}

/// Stop a hands-free recording from a single Fn tap.
///
/// Once hands-free mode is engaged (via double-tap, or via toggle mode when the
/// persisted setting is on), the user expects ONE tap to end the recording —
/// not a second double-tap. The fnkey timer calls this from the key-release
/// path; the grace window there guarantees we never cancel the gesture that
/// just started the recording. Restores in-memory hands_free to the persisted
/// preference, mirroring [`handle_fn_double_tap`].
pub fn handle_fn_stop(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();

    let Ok(mut app_state) = state.try_lock() else {
        sentry::add_breadcrumb(sentry::Breadcrumb {
            category: Some("shortcuts".to_string()),
            message: Some("try_lock contended at handle_fn_stop".to_string()),
            level: sentry::Level::Warning,
            ..Default::default()
        });
        crate::logging::log_warn("[Shortcuts] try_lock contended at handle_fn_stop");
        crate::trace::event(
            "hotkey.event_dropped",
            serde_json::json!({ "at": "handle_fn_stop" }),
        );
        return;
    };

    if app_state.is_recording() {
        stop_recording(&mut app_state, app);
        // No explicit hands_free restore — the session override is cleared
        // by set_state on the Idle transition triggered from the pipeline,
        // so the persisted preference applies for the next session.
    }
}

/// Handle shortcut key press - implements double-tap detection and settings-based toggle mode
fn handle_shortcut_pressed(state: &mut AppState, app: &AppHandle) {
    let now = Instant::now();
    let settings = get_settings();
    let settings_hands_free = settings.hands_free_mode;

    let is_double_tap = state
        .last_shortcut_time
        .map(|last| now.duration_since(last).as_millis() < DOUBLE_TAP_THRESHOLD_MS)
        .unwrap_or(false);

    state.last_shortcut_time = Some(now);

    if is_double_tap {
        // Reset the candidate so the NEXT tap is treated as a fresh single
        // press rather than the third tap of a triple-tap. fnkey.rs has the
        // same guard via LAST_FN_PRESS_TIME_MS.store(0) and ours was missing,
        // which made every shortcut press within 300 ms of a successful
        // double-tap re-fire the double-tap path.
        state.last_shortcut_time = None;
        match state.recording_state {
            RecordingState::Idle => {
                // Transient: hands-free for this session only via the
                // session override. The persisted preference is untouched
                // and the override clears automatically on Idle.
                state.enter_hands_free_session();
                start_recording(state, app);
            }
            RecordingState::Recording if state.effective_hands_free() => {
                stop_recording(state, app);
            }
            _ => {}
        }
    } else {
        // Keep the persisted preference in sync with the settings cache so
        // any UI change made while we were idle is reflected now.
        state.set_persistent_hands_free(settings_hands_free);
        if state.is_idle() {
            start_recording(state, app);
        } else if state.is_recording() && state.effective_hands_free() {
            // Single press while recording in hands-free mode → stop
            stop_recording(state, app);
        }
    }
}

/// Handle shortcut key release - stops push-to-talk recording
fn handle_shortcut_released(state: &mut AppState, app: &AppHandle) {
    if !state.effective_hands_free() && state.is_recording() {
        stop_recording(state, app);
    }
}

/// Start recording: update state, show pill, play sound
fn start_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Recording, app);
    set_recording_icon(app, true);
    show_pill(app);
    play_start_sound(app);
    // Tell the Fn monitor whether this is a hands-free recording, so a single
    // Fn tap can stop it (see fnkey::handle_fn_stop). Push-to-talk recordings
    // are unaffected.
    #[cfg(target_os = "macos")]
    crate::fnkey::set_hands_free_recording(state.effective_hands_free());
}

/// Stop recording: update state to Processing, play sound
/// Pill visibility is determined by should_show_pill() during Processing state
fn stop_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Processing, app);
    set_recording_icon(app, false);
    play_stop_sound(app);
    #[cfg(target_os = "macos")]
    crate::fnkey::set_hands_free_recording(false);
    // During Processing, show pill if setting allows (shows during processing regardless of hide setting)
    if should_show_pill(app) {
        show_pill(app);
    } else {
        hide_pill(app);
    }
}
