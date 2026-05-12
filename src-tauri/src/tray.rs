// TTP - Talk To Paste
// System tray setup and management

use crate::settings::get_settings;
use crate::sounds::{play_start_sound, play_stop_sound};
use crate::state::{AppState, RecordingState};
use std::sync::{Mutex, OnceLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Listener, Manager,
};

/// Cached PNG bytes of the warning-state idle icon (icon-idle.png with a
/// red dot composited in the bottom-right corner). Generated lazily on the
/// first request and reused for the rest of the session.
static WARNING_ICON_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

/// Build the warning-state tray icon by overlaying a red dot on
/// icon-idle.png. Returns PNG bytes ready for `Image::from_bytes`. Falls
/// back to the original idle bytes on encode failure so the tray never
/// goes blank.
fn warning_idle_icon_bytes() -> &'static [u8] {
    const IDLE_BYTES: &[u8] = include_bytes!("../icons/icon-idle.png");
    WARNING_ICON_BYTES.get_or_init(|| {
        match image::load_from_memory(IDLE_BYTES) {
            Ok(img) => {
                let mut rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                // Red-600 (#dc2626). Dot sized for a 44x44 retina tray icon;
                // scales proportionally to other sizes.
                let dot_r = (w.min(h) as i32) / 5;
                let cx = w as i32 - dot_r - 2;
                let cy = h as i32 - dot_r - 2;
                let r_sq = dot_r * dot_r;
                let outer_sq = (dot_r + 1) * (dot_r + 1);
                for y in 0..h as i32 {
                    for x in 0..w as i32 {
                        let dx = x - cx;
                        let dy = y - cy;
                        let d_sq = dx * dx + dy * dy;
                        if d_sq <= r_sq {
                            rgba.put_pixel(x as u32, y as u32, image::Rgba([220, 38, 38, 255]));
                        } else if d_sq <= outer_sq {
                            // 1-pixel-wide soft edge for anti-aliasing.
                            rgba.put_pixel(x as u32, y as u32, image::Rgba([220, 38, 38, 160]));
                        }
                    }
                }
                let mut buf = Vec::new();
                let encoder = image::codecs::png::PngEncoder::new(&mut buf);
                use image::ImageEncoder;
                if encoder
                    .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
                    .is_ok()
                {
                    buf
                } else {
                    IDLE_BYTES.to_vec()
                }
            }
            Err(_) => IDLE_BYTES.to_vec(),
        }
    })
}

/// True when Fn is the active hotkey AND Input Monitoring is missing.
/// Centralised so the icon picker and the menu builder agree on when to
/// surface the warning state.
#[cfg(target_os = "macos")]
fn input_monitoring_warning_active() -> bool {
    get_settings().fn_key_enabled && !crate::fnkey::has_input_monitoring()
}
#[cfg(not(target_os = "macos"))]
fn input_monitoring_warning_active() -> bool {
    false
}

/// Bytes of the idle icon to display in the current state. Returns the
/// warning variant if Input Monitoring is missing for the Fn hotkey,
/// otherwise the plain idle icon.
fn idle_icon_bytes() -> &'static [u8] {
    const IDLE_BYTES: &[u8] = include_bytes!("../icons/icon-idle.png");
    if input_monitoring_warning_active() {
        warning_idle_icon_bytes()
    } else {
        IDLE_BYTES
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Build context menu (including the conditional permission-warning entry
    // when Fn is configured but Input Monitoring is missing).
    let menu = build_tray_menu(app, false)?;

    // Pick the idle icon variant for the current permission state — shows
    // a red dot overlay when Input Monitoring is missing for the Fn hotkey.
    let tray_icon = Image::from_bytes(idle_icon_bytes())
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
            "fix_input_monitoring" => {
                // Open macOS Privacy & Security → Input Monitoring directly.
                // Same deep link as the in-Settings button; here we just
                // surface it from the tray for users who never open Settings.
                use tauri_plugin_opener::OpenerExt;
                let _ = app.opener().open_url(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
                    None::<&str>,
                );
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Toggle recording state from tray menu
fn toggle_recording(app: &AppHandle) {
    // Compute the transition inside the lock, then drop the lock BEFORE
    // performing side effects (set_state emits events + starts audio monitor,
    // and the icon/pill/sound/menu helpers can re-enter Tauri). Holding the
    // AppState mutex across those calls caused deadlocks (see Sentry TTP-A).
    let next_state = {
        let state = app.state::<Mutex<AppState>>();
        let Ok(mut app_state) = state.try_lock() else {
            eprintln!("[Tray] Could not acquire state lock");
            return;
        };

        match app_state.recording_state {
            RecordingState::Idle => {
                app_state.hands_free_mode = true; // Use hands-free mode for tray
                app_state.set_state(RecordingState::Recording, app);
                RecordingState::Recording
            }
            RecordingState::Recording => {
                app_state.set_state(RecordingState::Processing, app);
                RecordingState::Processing
            }
            RecordingState::Processing => RecordingState::Processing,
        }
    }; // ← Mutex released here, before any UI/sound side effects

    match next_state {
        RecordingState::Recording => {
            set_recording_icon(app, true);
            show_pill(app);
            play_start_sound(app);
            update_tray_menu(app, true);
        }
        RecordingState::Processing => {
            set_recording_icon(app, false);
            // Keep pill visible during processing, hide when done
            play_stop_sound(app);
            update_tray_menu(app, false);
        }
        RecordingState::Idle => {}
    }
}

/// Update tray menu text based on recording state. Also re-evaluates the
/// Input Monitoring permission state — if the user has granted it since
/// the previous build, the warning entry disappears at the next state
/// transition (start/stop recording).
fn update_tray_menu(app: &AppHandle, is_recording: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(menu) = build_tray_menu(app, is_recording) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Build the tray context menu. Always contains record / settings / quit;
/// prepends a "⚠ Fix Input Monitoring permission" entry when Fn is the
/// configured hotkey and the OS hasn't granted that permission yet — the
/// only silent-failure case in the app, so the most important one to
/// surface where the user actually looks (the tray, not buried in Settings).
fn build_tray_menu(
    app: &AppHandle,
    is_recording: bool,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let record_text = if is_recording {
        "Stop Recording"
    } else {
        "Start Recording"
    };
    let record = MenuItem::with_id(app, "record", record_text, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TTP", true, None::<&str>)?;

    // Only show the permission warning when Fn is the active hotkey AND
    // the OS hasn't granted Input Monitoring. Other hotkeys don't depend
    // on this permission, so the warning would be misleading noise.
    let needs_input_monitoring = {
        #[cfg(target_os = "macos")]
        {
            get_settings().fn_key_enabled && !crate::fnkey::has_input_monitoring()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    };

    if needs_input_monitoring {
        let warn = MenuItem::with_id(
            app,
            "fix_input_monitoring",
            "⚠ Fix Input Monitoring permission",
            true,
            None::<&str>,
        )?;
        let warn_separator = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(
            app,
            &[&warn, &warn_separator, &record, &separator, &settings, &quit],
        )?;
        Ok(menu)
    } else {
        let menu = Menu::with_items(app, &[&record, &separator, &settings, &quit])?;
        Ok(menu)
    }
}

/// Update the tray icon to reflect recording state. When not recording,
/// uses the warning variant if Input Monitoring is missing for the Fn
/// hotkey — so the red dot stays visible as soon as a recording ends.
pub fn set_recording_icon(app: &AppHandle, recording: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        let icon_bytes: &[u8] = if recording {
            include_bytes!("../icons/icon-recording.png")
        } else {
            idle_icon_bytes()
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
        // Re-apply after a short delay to ensure macOS window server has processed it.
        // Use Tauri's async runtime so the Tauri API call lands on a runtime-attached
        // worker thread (mirrors the sounds.rs fix — avoids "no reactor running" panic
        // if set_ignore_cursor_events ever delegates to Tokio internals on macOS).
        let w = window.clone();
        tauri::async_runtime::spawn_blocking(move || {
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
