// TTP - Talk To Paste
// Global keyboard shortcut handling with push-to-talk and double-tap toggle

use crate::settings::get_settings;
use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use crate::tray::set_recording_icon;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Double-tap detection threshold in milliseconds
const DOUBLE_TAP_THRESHOLD_MS: u128 = 300;

/// Set up global keyboard shortcuts for recording control
pub fn setup_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Load shortcut from settings, fall back to default if invalid
    let settings = get_settings();
    let shortcut_str = settings.shortcut;

    let shortcut = shortcut_str.parse::<Shortcut>().unwrap_or_else(|_| {
        println!("Invalid shortcut '{}', falling back to Alt+Space", shortcut_str);
        "Alt+Space".parse::<Shortcut>().unwrap()
    });

    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(shortcut.clone(), move |_app, _shortcut, event| {
        handle_shortcut_event(&app_handle, event.state);
    })?;

    println!("Global shortcut {} registered", shortcut_str);
    Ok(())
}

/// Update the global shortcut at runtime
pub fn update_shortcut(app: &AppHandle, new_shortcut: &str) -> Result<(), String> {
    // Parse the new shortcut string
    let shortcut = new_shortcut.parse::<Shortcut>()
        .map_err(|e| format!("Invalid shortcut format '{}': {}", new_shortcut, e))?;

    let global_shortcut = app.global_shortcut();

    // Unregister all existing shortcuts
    global_shortcut.unregister_all()
        .map_err(|e| format!("Failed to unregister existing shortcuts: {}", e))?;

    // Register the new shortcut
    let app_handle = app.clone();
    global_shortcut.on_shortcut(shortcut, move |_app, _shortcut, event| {
        handle_shortcut_event(&app_handle, event.state);
    }).map_err(|e| format!("Failed to register shortcut '{}': {}", new_shortcut, e))?;

    println!("Global shortcut updated to {}", new_shortcut);
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

/// Stop recording: update state to Processing, play sound
/// The frontend will stop mic recording and call process_audio command
/// Pipeline will set state back to Idle when complete
fn stop_recording(state: &mut AppState, app: &AppHandle) {
    state.set_state(RecordingState::Processing, app);

    // Update tray icon
    set_recording_icon(app, false);

    // Play stop sound
    play_stop_sound(app);
}
