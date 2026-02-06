// TTP - Talk To Paste
// Global keyboard shortcut handling with push-to-talk and double-tap toggle

use crate::settings::get_settings;
use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use crate::tray::{set_recording_icon, show_pill};
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

    let shortcut = shortcut_str.parse::<Shortcut>().unwrap_or_else(|_| {
        println!("Invalid shortcut '{}', using Alt+Space", shortcut_str);
        "Alt+Space".parse::<Shortcut>().unwrap()
    });

    // Use register() instead of on_shortcut() - handler is set in Builder
    app.global_shortcut().register(shortcut)?;

    println!("Global shortcut {} registered", shortcut_str);
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

    println!("Global shortcut updated to {}", new_shortcut);
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

/// Handle shortcut key press - implements double-tap detection
fn handle_shortcut_pressed(state: &mut AppState, app: &AppHandle) {
    let now = Instant::now();

    let is_double_tap = state
        .last_shortcut_time
        .map(|last| now.duration_since(last).as_millis() < DOUBLE_TAP_THRESHOLD_MS)
        .unwrap_or(false);

    state.last_shortcut_time = Some(now);

    if is_double_tap {
        match state.recording_state {
            RecordingState::Idle => {
                state.hands_free_mode = true;
                start_recording(state, app);
            }
            RecordingState::Recording if state.hands_free_mode => {
                stop_recording(state, app);
                state.hands_free_mode = false;
            }
            _ => {}
        }
    } else {
        if state.is_idle() {
            state.hands_free_mode = false;
            start_recording(state, app);
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
fn stop_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Processing, app);
    set_recording_icon(app, false);
    play_stop_sound(app);
}
