// TTP - Talk To Paste
// System tray setup and management

use crate::settings::get_settings;
use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Listener, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let record = MenuItem::with_id(app, "record", "Start Recording", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TTP", true, None::<&str>)?;

    // Build context menu
    let menu = Menu::with_items(app, &[&record, &separator, &settings, &quit])?;

    // Use simple tray icon (monochrome, works with macOS template)
    let tray_icon = Image::from_bytes(include_bytes!("../icons/icon-idle.png"))
        .map_err(|e| format!("Failed to load tray icon: {}", e))?;

    // Build tray icon with ID for later reference
    let _tray = TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(true) // Left-click or right-click for menu
        .tooltip("TTP by AmirKS — Talk To Paste")
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
            app_state.hands_free_mode = true; // Use hands-free mode for tray
            app_state.set_state(RecordingState::Recording, app);
            set_recording_icon(app, true);
            show_pill(app);
            play_start_sound(app);
            update_tray_menu(app, true);
        }
        RecordingState::Recording => {
            app_state.set_state(RecordingState::Processing, app);
            set_recording_icon(app, false);
            // Keep pill visible during processing, hide when done
            play_stop_sound(app);
            update_tray_menu(app, false);
        }
        RecordingState::Processing => {}
    }
}

/// Update tray menu text based on recording state
fn update_tray_menu(app: &AppHandle, is_recording: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        if let Some(menu) = tray.menu() {
            if let Some(item) = menu.get("record") {
                let text = if is_recording {
                    "Stop Recording"
                } else {
                    "Start Recording"
                };
                let _ = item.as_menuitem().map(|mi| mi.set_text(text));
            }
        }
    }
}

/// Update the tray icon to reflect recording state
pub fn set_recording_icon(app: &AppHandle, recording: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        let icon_bytes: &[u8] = if recording {
            include_bytes!("../icons/icon-recording.png")
        } else {
            include_bytes!("../icons/icon-idle.png")
        };
        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

/// Show the pill window (floating recording indicator)
pub fn show_pill(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("pill") {
        // Center horizontally, just above the dock/taskbar
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let scale = monitor.scale_factor();
            let screen_width = monitor.size().width as f64 / scale;
            let screen_height = monitor.size().height as f64 / scale;
            let window_width = 380.0;
            let x = (screen_width - window_width) / 2.0;
            // Window is 100px tall, content is bottom-aligned (justify-end)
            // Position so the pill sits ~80px above screen bottom (above dock)
            #[cfg(target_os = "macos")]
            let y = screen_height - 192.0;
            #[cfg(target_os = "windows")]
            let y = screen_height - 150.0;
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let y = screen_height - 150.0;
            let _ = window.set_position(tauri::LogicalPosition::new(x, y));
        }
        // Set click-through BEFORE showing to avoid race on macOS
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        // Re-apply after a short delay to ensure macOS window server has processed it
        let w = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = w.set_ignore_cursor_events(true);
        });
    }
}

/// Hide the pill window
pub fn hide_pill(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("pill") {
        let _ = window.hide();
    }
}

/// Determine if the pill should be visible based on recording state and settings
/// - Shows during active recording regardless of setting
/// - Shows during processing (including errors) regardless of setting
/// - Shows during idle if hide_pill_when_inactive is false
/// - Hides during idle if hide_pill_when_inactive is true
pub fn should_show_pill(app: &AppHandle) -> bool {
    let state = match app.try_state::<Mutex<AppState>>() {
        Some(s) => s,
        None => return true,
    };

    let Ok(app_state) = state.try_lock() else {
        // Mutex already locked (called from set_state) — fall back to settings-only check
        return should_show_pill_for_state(&RecordingState::Idle);
    };

    should_show_pill_for_state(&app_state.recording_state)
}

/// Check pill visibility based on a known recording state (no mutex needed).
/// Called from set_state() where the mutex is already held.
pub fn should_show_pill_for_state(recording_state: &RecordingState) -> bool {
    // Show during recording and processing (including errors)
    if *recording_state == RecordingState::Recording || *recording_state == RecordingState::Processing {
        return true;
    }

    // Check setting for idle state
    let settings = get_settings();
    !settings.hide_pill_when_inactive
}

/// Set up listener for settings changes to update pill visibility and hands-free mode
pub fn setup_settings_listener(app: &AppHandle) {
    let app_handle = app.clone();
    app.listen("settings-changed", move |_event| {
        // Update pill visibility based on new settings
        if should_show_pill(&app_handle) {
            show_pill(&app_handle);
        } else {
            hide_pill(&app_handle);
        }

        // Sync hands_free_mode from settings to AppState
        let settings = get_settings();
        if let Some(state) = app_handle.try_state::<Mutex<AppState>>() {
            if let Ok(mut app_state) = state.try_lock() {
                // Only update if not currently recording (avoid disrupting active session)
                if app_state.is_idle() {
                    app_state.hands_free_mode = settings.hands_free_mode;
                }
            }
        }
    });
}
