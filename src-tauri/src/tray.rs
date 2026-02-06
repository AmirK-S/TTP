// TTP - Talk To Paste
// System tray setup and management

use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let record = MenuItem::with_id(app, "record", "Start Recording", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TTP", true, None::<&str>)?;

    // Build context menu
    let menu = Menu::with_items(app, &[&record, &separator, &settings, &quit])?;

    // Use default icon (configured in tauri.conf.json trayIcon)
    let icon = app.default_window_icon().cloned().ok_or("No default icon")?;

    // Build tray icon with ID for later reference
    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false) // Right-click for menu
        .tooltip("TTP - Talk To Paste")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "record" => {
                // Toggle recording via tray menu
                println!("[Tray] Record clicked!");
                toggle_recording(app);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Toggle recording state from tray menu
fn toggle_recording(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();

    let Ok(mut app_state) = state.try_lock() else {
        eprintln!("[Tray] Could not acquire state lock");
        return;
    };

    match app_state.recording_state {
        RecordingState::Idle => {
            // Start recording
            println!("[Tray] Starting recording...");
            app_state.hands_free_mode = true; // Use hands-free mode for tray
            app_state.set_state(RecordingState::Recording, app);
            set_recording_icon(app, true);
            show_pill(app);
            play_start_sound(app);
            update_tray_menu(app, true);
        }
        RecordingState::Recording => {
            // Stop recording
            println!("[Tray] Stopping recording...");
            app_state.set_state(RecordingState::Processing, app);
            set_recording_icon(app, false);
            // Keep pill visible during processing, hide when done
            play_stop_sound(app);
            update_tray_menu(app, false);
        }
        RecordingState::Processing => {
            println!("[Tray] Already processing, please wait...");
        }
    }
}

/// Update tray menu text based on recording state
fn update_tray_menu(_app: &AppHandle, _is_recording: bool) {
    // TODO: Update menu text dynamically if needed
    // For now, the menu item text stays static
}

/// Update the tray icon to reflect recording state
/// Note: For icon changes, we'll use the default icon for now
/// Custom icons will be added when proper icon loading is implemented
pub fn set_recording_icon(app: &AppHandle, _recording: bool) {
    // TODO: Implement custom icon switching when icons are embedded as resources
    // For now, tray icon remains static (configured in tauri.conf.json)
    if let Some(tray) = app.tray_by_id("main") {
        if let Some(icon) = app.default_window_icon().cloned() {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

/// Show the pill window (floating recording indicator)
pub fn show_pill(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("pill") {
        // Center horizontally, well above the dock (300px from bottom)
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let screen_width = monitor.size().width as i32;
            let screen_height = monitor.size().height as i32;
            let window_width = 160;
            let x = (screen_width - window_width) / 2;
            let y = screen_height - 350; // Well above the dock
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        }
        let _ = window.show();
        let _ = window.set_always_on_top(true);
    }
}

/// Hide the pill window
pub fn hide_pill(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("pill") {
        let _ = window.hide();
    }
}
