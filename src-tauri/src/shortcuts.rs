// TTP - Talk To Paste
// Global keyboard shortcut handling with push-to-talk and double-tap toggle

use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use crate::tray::set_recording_icon;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Double-tap detection threshold in milliseconds
const DOUBLE_TAP_THRESHOLD_MS: u128 = 300;

/// Set up global keyboard shortcuts for recording control
pub fn setup_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Register Option+Space as the global shortcut
    let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);

    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        handle_shortcut_event(&app_handle, event.state);
    })?;

    println!("Global shortcut Option+Space registered");
    Ok(())
}

/// Handle shortcut event - dispatches to press/release handlers
fn handle_shortcut_event(app: &AppHandle, shortcut_state: ShortcutState) {
    let state = app.state::<Mutex<AppState>>();

    // Use try_lock to avoid blocking the main thread
    let Ok(mut app_state) = state.try_lock() else {
        eprintln!("Warning: Could not acquire state lock for shortcut handler");
        return;
    };

    match shortcut_state {
        ShortcutState::Pressed => {
            handle_shortcut_pressed(&mut app_state, app);
        }
        ShortcutState::Released => {
            handle_shortcut_released(&mut app_state, app);
        }
    }
}

/// Handle shortcut key press - implements double-tap detection
fn handle_shortcut_pressed(state: &mut AppState, app: &AppHandle) {
    let now = Instant::now();

    // Check for double-tap
    let is_double_tap = state
        .last_shortcut_time
        .map(|last| now.duration_since(last).as_millis() < DOUBLE_TAP_THRESHOLD_MS)
        .unwrap_or(false);

    state.last_shortcut_time = Some(now);

    if is_double_tap {
        // Double-tap: toggle hands-free mode
        match state.recording_state {
            RecordingState::Idle => {
                // Start hands-free recording
                state.hands_free_mode = true;
                start_recording(state, app);
                println!("Double-tap: Started hands-free recording");
            }
            RecordingState::Recording if state.hands_free_mode => {
                // Stop hands-free recording
                stop_recording(state, app);
                state.hands_free_mode = false;
                println!("Double-tap: Stopped hands-free recording");
            }
            _ => {
                // Ignore double-tap during processing or non-hands-free recording
            }
        }
    } else {
        // Single press: push-to-talk mode
        if state.is_idle() {
            state.hands_free_mode = false;
            start_recording(state, app);
            println!("Push-to-talk: Started recording");
        }
    }
}

/// Handle shortcut key release - stops push-to-talk recording
fn handle_shortcut_released(state: &mut AppState, app: &AppHandle) {
    // Only stop if in push-to-talk mode (not hands-free)
    if !state.hands_free_mode && state.is_recording() {
        stop_recording(state, app);
        println!("Push-to-talk: Stopped recording");
    }
}

/// Start recording: update state, play sound
fn start_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Recording, app);

    // Update tray icon
    set_recording_icon(app, true);

    // Play start sound
    play_start_sound(app);
}

/// Stop recording: update state, play sound
fn stop_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Idle, app);

    // Update tray icon
    set_recording_icon(app, false);

    // Play stop sound
    play_stop_sound(app);
}
