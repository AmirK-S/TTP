// TTP - Talk To Paste
// Main Tauri application entry point

mod audio_monitor;
mod credentials;
mod dictionary;
#[cfg(target_os = "macos")]
mod fnkey;
mod history;
pub mod logging;
mod paste;
mod recording;
mod settings;
mod shortcuts;
mod sounds;
mod state;
mod telemetry;
mod transcription;
mod tray;

use credentials::{
    delete_groq_api_key, get_groq_api_key, has_groq_api_key, set_groq_api_key,
};
use dictionary::{add_dictionary_entry, clear_dictionary, delete_dictionary_entry, get_dictionary};
use history::{clear_history, get_history};
use recording::{get_recordings_dir, RecordingContext};
use settings::{get_settings, reset_settings, set_settings};
use transcription::process_audio;
use state::AppState;
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// Tauri command to update the global shortcut at runtime
#[tauri::command]
fn update_shortcut_cmd(app: AppHandle, shortcut: String) -> Result<(), String> {
    shortcuts::update_shortcut(&app, &shortcut)
}

/// Tauri command to unregister all global shortcuts (used when switching to Fn mode)
#[tauri::command]
fn unregister_shortcuts_cmd(app: AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;
    Ok(())
}

/// Tauri command to toggle Fn key monitoring
#[tauri::command]
fn set_fn_key_enabled(enabled: bool) {
    #[cfg(target_os = "macos")]
    fnkey::set_fn_key_enabled(enabled);
    #[cfg(not(target_os = "macos"))]
    { let _ = enabled; }
}

/// Check and request Input Monitoring permission (needed for Fn key detection)
#[tauri::command]
fn check_input_monitoring() -> bool {
    #[cfg(target_os = "macos")]
    {
        if fnkey::has_input_monitoring() {
            return true;
        }
        fnkey::request_input_monitoring()
    }
    #[cfg(not(target_os = "macos"))]
    { true }
}

/// Tauri command to reset state to Idle (used when skipping short recordings)
#[tauri::command]
fn reset_to_idle(app: AppHandle) {
    if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = state.try_lock() {
            guard.set_state(state::RecordingState::Idle, &app);
            tray::set_recording_icon(&app, false);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use std::sync::Arc;

    // --- Sentry init (MUST be before tauri::Builder) ---
    // When telemetry is disabled (default), DSN is None and Sentry makes zero network requests.
    let dsn = telemetry::consent::get_sentry_dsn();
    let telemetry_active = dsn.is_some();

    let client = sentry::init(sentry::ClientOptions {
        dsn,
        release: sentry::release_name!(),
        environment: Some(
            if cfg!(debug_assertions) { "development" } else { "production" }.into(),
        ),
        auto_session_tracking: telemetry_active,
        send_default_pii: false,
        before_send: Some(Arc::new(|mut event| {
            telemetry::sentry::scrub_event_pii(&mut event);
            Some(event)
        })),
        before_breadcrumb: Some(Arc::new(|mut breadcrumb| {
            telemetry::sentry::scrub_breadcrumb_pii(&mut breadcrumb);
            Some(breadcrumb)
        })),
        ..Default::default()
    });

    // Minidump handler for native crashes (segfaults, stack overflows)
    // Only init when telemetry is active — minidump re-executes the binary
    // as a crash reporter process, which causes a duplicate app in dev mode
    let _minidump_guard = if telemetry_active {
        Some(tauri_plugin_sentry::minidump::init(&client))
    } else {
        None
    };

    // --- Tauri app (runs only in main process after minidump init) ---
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_sentry::init(&client))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    shortcuts::handle_shortcut_event_public(app, event.state());
                })
                .build(),
        )
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_mic_recorder::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    // Note: Aptabase plugin is registered inside setup() because it requires
    // a Tokio runtime context for HTTP client creation (which doesn't exist yet here).

    builder
        .manage(Mutex::new(AppState::default()))
        .manage(Mutex::new(RecordingContext::default()))
        .setup(move |app| {
            // Register Aptabase analytics plugin
            // Must be inside setup AND within a Tokio runtime context because
            // the plugin spawns a polling task via tokio::spawn during init.
            if telemetry_active {
                let handle = app.handle().clone();
                tauri::async_runtime::block_on(async move {
                    let _ = handle.plugin(
                        tauri_plugin_aptabase::Builder::new(telemetry::analytics::APTABASE_APP_KEY).build(),
                    );
                    handle.manage(telemetry::analytics::TelemetryActive);
                });
            }

            // Clean up stale audio backups (>24 hours old)
            transcription::backup::cleanup_stale_backups(app.handle());

            // Hide from dock — TTP is a tray-only app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Check accessibility permission (needed for paste simulation on macOS)
            #[cfg(target_os = "macos")]
            {
                if !paste::check_accessibility() {
                    // Send notification instead of opening System Preferences every time
                    let _ = app.handle().emit("accessibility-missing", ());
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app.notification()
                        .builder()
                        .title("TTP — Accessibility Required")
                        .body("TTP needs Accessibility permission to paste text. Go to System Settings → Privacy & Security → Accessibility and enable TTP.")
                        .show();
                }
            }

            // Set up global keyboard shortcuts
            shortcuts::setup_shortcuts(app.handle())?;

            // Start Fn key monitor (macOS only, always running but toggled via settings)
            #[cfg(target_os = "macos")]
            {
                fnkey::start_fn_key_monitor(app.handle());
                let fn_enabled = settings::get_settings().fn_key_enabled;
                fnkey::set_fn_key_enabled(fn_enabled);
            }

            // Show pill window (always visible)
            tray::show_pill(app.handle());

            // Check if Groq API key exists, show setup window if not
            let has_groq = credentials::get_groq_api_key_internal(app.handle())
                .map(|k| k.is_some())
                .unwrap_or(false);

            if !has_groq {
                // Show setup window for first-run experience
                if let Some(window) = app.get_webview_window("setup") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Prevent app from quitting when setup/settings windows close
            // TTP is a tray app — it should keep running in background
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_groq_api_key,
            set_groq_api_key,
            has_groq_api_key,
            delete_groq_api_key,
            get_recordings_dir,
            process_audio,
            get_settings,
            set_settings,
            reset_settings,
            get_dictionary,
            add_dictionary_entry,
            delete_dictionary_entry,
            clear_dictionary,
            get_history,
            clear_history,
            update_shortcut_cmd,
            unregister_shortcuts_cmd,
            set_fn_key_enabled,
            check_input_monitoring,
            reset_to_idle
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |handler, event| {
            match event {
                tauri::RunEvent::Ready { .. } => {
                    telemetry::analytics::track(handler, "app_started", None);
                }
                tauri::RunEvent::Exit { .. } => {
                    if telemetry_active {
                        use tauri_plugin_aptabase::EventTracker;
                        handler.flush_events_blocking();
                    }
                }
                _ => {}
            }
        });
}
