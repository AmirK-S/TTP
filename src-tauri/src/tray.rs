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

/// Cached PNG bytes of each composited idle-icon variant. Built lazily on
/// first use and reused for the rest of the session. Four cells: warning
/// only (red dot bottom-right), update only (blue dot top-right), both,
/// and the plain idle bytes inline since they're already static.
static WARNING_ICON_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
static UPDATE_ICON_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
static WARNING_UPDATE_ICON_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

/// Version string of a pending update that has already been downloaded +
/// installed in the background (the .app bundle on disk is already the new
/// version, but the running process is still old). When `Some`, the tray
/// shows a blue dot + "Install update" menu item; clicking it relaunches
/// into the new binary. Reset to `None` once the update is consumed.
static PENDING_UPDATE_VERSION: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[derive(Clone, Copy)]
enum DotPosition {
    BottomRight,
    TopRight,
}

#[derive(Clone, Copy)]
struct DotConfig {
    color: [u8; 4],
    position: DotPosition,
}

const RED_DOT: DotConfig = DotConfig {
    color: [220, 38, 38, 255],
    position: DotPosition::BottomRight,
};
const BLUE_DOT: DotConfig = DotConfig {
    color: [37, 99, 235, 255],
    position: DotPosition::TopRight,
};

/// Composite a list of colored dots onto icon-idle.png. Returns the encoded
/// PNG bytes ready for `Image::from_bytes`. Falls back to the original idle
/// bytes if anything in the pipeline fails so the tray never goes blank.
fn compose_idle_icon(dots: &[DotConfig]) -> Vec<u8> {
    const IDLE_BYTES: &[u8] = include_bytes!("../icons/icon-idle.png");
    let img = match image::load_from_memory(IDLE_BYTES) {
        Ok(img) => img,
        Err(_) => return IDLE_BYTES.to_vec(),
    };
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    // Dot sized for a 44x44 retina tray icon; scales with image dimensions.
    let dot_r = (w.min(h) as i32) / 5;
    let r_sq = dot_r * dot_r;
    let outer_sq = (dot_r + 1) * (dot_r + 1);
    for dot in dots {
        let (cx, cy) = match dot.position {
            DotPosition::BottomRight => (w as i32 - dot_r - 2, h as i32 - dot_r - 2),
            DotPosition::TopRight => (w as i32 - dot_r - 2, dot_r + 2),
        };
        let solid = image::Rgba(dot.color);
        let soft = image::Rgba([dot.color[0], dot.color[1], dot.color[2], 160]);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let dx = x - cx;
                let dy = y - cy;
                let d_sq = dx * dx + dy * dy;
                if d_sq <= r_sq {
                    rgba.put_pixel(x as u32, y as u32, solid);
                } else if d_sq <= outer_sq {
                    // 1-pixel-wide soft edge for anti-aliasing.
                    rgba.put_pixel(x as u32, y as u32, soft);
                }
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

fn warning_idle_icon_bytes() -> &'static [u8] {
    WARNING_ICON_BYTES.get_or_init(|| compose_idle_icon(&[RED_DOT]))
}

fn update_idle_icon_bytes() -> &'static [u8] {
    UPDATE_ICON_BYTES.get_or_init(|| compose_idle_icon(&[BLUE_DOT]))
}

fn warning_update_idle_icon_bytes() -> &'static [u8] {
    WARNING_UPDATE_ICON_BYTES.get_or_init(|| compose_idle_icon(&[RED_DOT, BLUE_DOT]))
}

fn pending_update_lock() -> &'static Mutex<Option<String>> {
    PENDING_UPDATE_VERSION.get_or_init(|| Mutex::new(None))
}

fn update_ready_active() -> bool {
    pending_update_lock()
        .lock()
        .ok()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

fn pending_update_version() -> Option<String> {
    pending_update_lock().lock().ok().and_then(|g| g.clone())
}

/// Record that an update has been silently downloaded + installed and is
/// waiting for the user to relaunch. Pass `None` to clear (e.g. after the
/// app has been relaunched and the new binary is now running).
pub fn set_pending_update(version: Option<String>) {
    if let Ok(mut g) = pending_update_lock().lock() {
        *g = version;
    }
}

/// Re-evaluate the tray icon and menu without changing recording state.
/// Called from the JS side after a silent install completes so the new
/// "Install update" menu item appears immediately.
pub fn refresh_tray(app: &AppHandle) {
    let is_recording = app
        .try_state::<Mutex<AppState>>()
        .and_then(|state| {
            state
                .try_lock()
                .ok()
                .map(|guard| guard.recording_state == RecordingState::Recording)
        })
        .unwrap_or(false);
    set_recording_icon(app, is_recording);
    update_tray_menu(app, is_recording);
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

/// True when Accessibility permission is missing — required for paste
/// simulation, so it blocks the core workflow regardless of hotkey config.
#[cfg(target_os = "macos")]
fn accessibility_warning_active() -> bool {
    !crate::paste::check_accessibility()
}
#[cfg(not(target_os = "macos"))]
fn accessibility_warning_active() -> bool {
    false
}

/// True when any required permission is missing — drives the red-dot icon
/// overlay.
fn permission_warning_active() -> bool {
    accessibility_warning_active() || input_monitoring_warning_active()
}

/// Bytes of the idle icon to display in the current state. Returns the
/// warning variant if any required permission is missing, otherwise the
/// plain idle icon.
fn idle_icon_bytes() -> &'static [u8] {
    const IDLE_BYTES: &[u8] = include_bytes!("../icons/icon-idle.png");
    let warn = permission_warning_active();
    let update = update_ready_active();
    match (warn, update) {
        (false, false) => IDLE_BYTES,
        (true, false) => warning_idle_icon_bytes(),
        (false, true) => update_idle_icon_bytes(),
        (true, true) => warning_update_idle_icon_bytes(),
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
        .tooltip(crate::i18n::tr("tray.tooltip"))
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
            "fix_accessibility" => {
                use tauri_plugin_opener::OpenerExt;
                let _ = app.opener().open_url(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                    None::<&str>,
                );
            }
            "install_update" => {
                // The update was already downloaded + installed silently in
                // the background; the .app bundle on disk is the new version.
                // All we need to do is relaunch into it. Reuse the same
                // LaunchServices-based restart path the in-app "Restart Now"
                // button uses so Gatekeeper doesn't block the relaunch.
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = crate::relaunch_app_via_launchservices(app);
                });
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
///
/// Also called from the settings-changed listener so a language switch
/// immediately rebuilds every menu label in the new locale (otherwise the
/// tray menu would only re-translate on the next start/stop transition).
fn update_tray_menu(app: &AppHandle, is_recording: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(menu) = build_tray_menu(app, is_recording) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Build the tray context menu. Always contains record / settings / quit;
/// prepends "Install update" and/or permission-fix entries when relevant.
/// We surface permission issues here because the tray is where users look
/// when something silently isn't working, not buried in Settings.
fn build_tray_menu(
    app: &AppHandle,
    is_recording: bool,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let record_text = if is_recording {
        crate::i18n::tr("tray.stopRecording")
    } else {
        crate::i18n::tr("tray.startRecording")
    };
    let record = MenuItem::with_id(app, "record", &record_text, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", crate::i18n::tr("tray.settings"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", crate::i18n::tr("tray.quit"), true, None::<&str>)?;

    let pending_version = pending_update_version();
    let needs_install_update = pending_version.is_some();
    let needs_accessibility = accessibility_warning_active();
    let needs_input_monitoring = input_monitoring_warning_active();

    // Build optional items first; keep them owned in named bindings so the
    // refs vector below can borrow into them (Tauri's Menu::with_items takes
    // `&[&dyn IsMenuItem<R>]` and we need stable addresses).
    let install_item = if needs_install_update {
        let label = crate::i18n::tr_with(
            "tray.installUpdate",
            &[("version", pending_version.as_deref().unwrap_or(""))],
        );
        Some(MenuItem::with_id(app, "install_update", &label, true, None::<&str>)?)
    } else {
        None
    };
    let install_sep = if needs_install_update {
        Some(PredefinedMenuItem::separator(app)?)
    } else {
        None
    };
    let accessibility_item = if needs_accessibility {
        Some(MenuItem::with_id(
            app,
            "fix_accessibility",
            crate::i18n::tr("tray.fixAccessibility"),
            true,
            None::<&str>,
        )?)
    } else {
        None
    };
    let input_mon_item = if needs_input_monitoring {
        Some(MenuItem::with_id(
            app,
            "fix_input_monitoring",
            crate::i18n::tr("tray.fixInputMonitoring"),
            true,
            None::<&str>,
        )?)
    } else {
        None
    };
    let perm_sep = if needs_accessibility || needs_input_monitoring {
        Some(PredefinedMenuItem::separator(app)?)
    } else {
        None
    };

    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    if let Some(i) = install_item.as_ref() { refs.push(i); }
    if let Some(s) = install_sep.as_ref() { refs.push(s); }
    if let Some(i) = accessibility_item.as_ref() { refs.push(i); }
    if let Some(i) = input_mon_item.as_ref() { refs.push(i); }
    if let Some(s) = perm_sep.as_ref() { refs.push(s); }
    refs.push(&record);
    refs.push(&separator);
    refs.push(&settings);
    refs.push(&quit);

    Ok(Menu::with_items(app, &refs)?)
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
///
/// Also rebuilds the tray menu on every settings change so a language switch
/// retranslates every label and tooltip immediately — without this the tray
/// menu would only pick up the new locale on the next recording transition.
/// The rebuild is cheap (it just re-runs `build_tray_menu`), so we don't
/// bother filtering on which setting actually changed.
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

        // Rebuild the tray menu so labels reflect the (possibly new) language.
        // Read the current recording state under a try_lock — if we can't get
        // the lock (e.g. mid-transition) we default to is_recording = false,
        // which matches the menu state shown to the user 99% of the time.
        let is_recording = app_handle
            .try_state::<Mutex<AppState>>()
            .and_then(|state| {
                state
                    .try_lock()
                    .ok()
                    .map(|guard| guard.recording_state == RecordingState::Recording)
            })
            .unwrap_or(false);
        update_tray_menu(&app_handle, is_recording);
    });
}
