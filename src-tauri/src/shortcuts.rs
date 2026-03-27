// TTP - Talk To Paste
// Global keyboard shortcut handling with push-to-talk and double-tap toggle

use crate::settings::{get_settings, set_settings};
use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use crate::tray::{set_recording_icon, should_show_pill, show_pill, hide_pill};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Double-tap detection threshold in milliseconds
const DOUBLE_TAP_THRESHOLD_MS: u128 = 300;

/// Persist hands_free_mode to settings when it changes
fn persist_hands_free_mode(app: &AppHandle, hands_free_mode: bool) {
    let mut settings = get_settings();
    settings.hands_free_mode = hands_free_mode;
    let _ = set_settings(settings, app.clone());
}

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
        { "Alt+Space".parse::<Shortcut>().unwrap() }
        #[cfg(not(target_os = "macos"))]
        { "Ctrl+Space".parse::<Shortcut>().unwrap() }
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
        return;
    };

    match app_state.recording_state {
        RecordingState::Idle => {
            app_state.hands_free_mode = true;
            start_recording(&mut app_state, app);
        }
        RecordingState::Recording if app_state.hands_free_mode => {
            stop_recording(&mut app_state, app);
            app_state.hands_free_mode = false;
        }
        _ => {}
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
        match state.recording_state {
            RecordingState::Idle => {
                state.hands_free_mode = true;
                if !settings_hands_free {
                    persist_hands_free_mode(app, true);
                }
                start_recording(state, app);
            }
            RecordingState::Recording if state.hands_free_mode => {
                stop_recording(state, app);
                state.hands_free_mode = settings_hands_free;
                if !settings_hands_free {
                    persist_hands_free_mode(app, false);
                }
            }
            _ => {}
        }
    } else {
        if state.is_idle() {
            // When settings has hands-free enabled, use toggle mode on single press
            state.hands_free_mode = settings_hands_free;
            start_recording(state, app);
        } else if state.is_recording() && state.hands_free_mode {
            // Single press while recording in hands-free mode → stop
            stop_recording(state, app);
            state.hands_free_mode = settings_hands_free;
        }
    }
}

/// Handle shortcut key release - stops push-to-talk recording
fn handle_shortcut_released(state: &mut AppState, app: &AppHandle) {
    if !state.hands_free_mode && state.is_recording() {
        stop_recording(state, app);
    }
}

/// Start recording: update state, show pill, play sound
fn start_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Recording, app);
    set_recording_icon(app, true);
    show_pill(app);
    play_start_sound(app);
}

/// Stop recording: update state to Processing, play sound
/// Pill visibility is determined by should_show_pill() during Processing state
fn stop_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Processing, app);
    set_recording_icon(app, false);
    play_stop_sound(app);
    // During Processing, show pill if setting allows (shows during processing regardless of hide setting)
    if should_show_pill(app) {
        show_pill(app);
    } else {
        hide_pill(app);
    }
}
