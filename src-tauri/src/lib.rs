// TTP - Talk To Paste
// Main Tauri application entry point

mod audio_monitor;
mod credentials;
mod dictionary;
#[cfg(target_os = "macos")]
mod fnkey;
mod history;
mod http_client;
mod i18n;
mod keychain;
mod licensing;
pub mod logging;
mod onboarding;
mod paste;
mod permissions;
mod recording;
mod settings;
mod shortcuts;
mod sounds;
mod state;
mod telemetry;
mod transcription;
mod tray;
mod usage;
mod whatsnew;

use credentials::{
    delete_groq_api_key, get_groq_api_key, has_groq_api_key, set_groq_api_key,
    validate_groq_api_key,
};
use dictionary::{add_dictionary_entry, clear_dictionary, delete_dictionary_entry, get_dictionary};
use history::{clear_history, get_history};
use licensing::{
    activate_license, deactivate_license, get_license_info, is_pro, validate_license,
};
use onboarding::{close_onboarding, show_onboarding};
use permissions::{
    check_microphone_permission, is_first_launch_cmd, mark_first_launch_complete_cmd,
    check_accessibility_permission, request_accessibility_permission,
    reset_accessibility_permission, PermissionStatus,
};
use recording::{get_recordings_dir, RecordingContext};
use settings::{get_settings, reset_settings, set_settings, open_settings_window};
use state::AppState;
use transcription::process_audio;
use usage::{get_analytics_summary, get_usage_stats};
use whatsnew::{check_whats_new, dismiss_whats_new};
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_updater::UpdaterExt;

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

/// Check Input Monitoring permission status WITHOUT triggering the system prompt.
/// Used by onboarding to display the current state of the checklist item.
/// On non-macOS, always returns true (no equivalent permission concept).
#[tauri::command]
fn check_input_monitoring_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        fnkey::has_input_monitoring()
    }
    #[cfg(not(target_os = "macos"))]
    { true }
}

/// Request Input Monitoring permission. On first call, this triggers the macOS
/// system prompt; if already denied, macOS silently returns false and the user
/// must toggle it in System Settings → Privacy & Security → Input Monitoring.
/// On non-macOS, always returns true (no-op).
#[tauri::command]
fn request_input_monitoring_permission() -> bool {
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

/// Pick the right manifest URL based on the user's channel preference.
fn channel_manifest_url(use_beta: bool) -> &'static str {
    if use_beta {
        "https://github.com/AmirK-S/TTP/releases/latest/download/latest-beta.json"
    } else {
        "https://github.com/AmirK-S/TTP/releases/latest/download/latest.json"
    }
}

/// Build a channel-aware Updater that targets either the stable or beta manifest.
fn build_channel_updater(
    app: &AppHandle,
    use_beta: bool,
) -> Result<tauri_plugin_updater::Updater, String> {
    let endpoint = channel_manifest_url(use_beta)
        .parse::<url::Url>()
        .map_err(|e| format!("Failed to parse update endpoint: {}", e))?;

    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to set update endpoints: {}", e))?
        .build()
        .map_err(|e| format!("Failed to build updater: {}", e))
}

/// Result of a channel-aware update check.
/// Tagged enum so the frontend can pattern-match cleanly.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum UpdateCheckResult {
    /// An update is available on the chosen channel. Frontend should prompt
    /// the user, then call `install_update_with_channel` to actually download
    /// and install it.
    Available { version: String, body: Option<String> },
    /// No update available — current build is up-to-date on this channel.
    NoUpdate,
}

/// Channel-aware update *check only*. Swaps the manifest URL based on the
/// user's `use_beta_channel` setting (stable: `latest.json`, beta:
/// `latest-beta.json`) and reports whether an update is available, without
/// downloading anything.
///
/// Splitting check vs install lets the frontend keep its existing two-step
/// UX (preview the version, then user clicks "Download and Install").
#[tauri::command]
async fn check_for_updates_with_channel(
    app: AppHandle,
    use_beta: bool,
) -> Result<UpdateCheckResult, String> {
    let updater = build_channel_updater(&app, use_beta)?;

    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateCheckResult::Available {
            version: update.version.clone(),
            body: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateCheckResult::NoUpdate),
        Err(e) => Err(format!("Update check failed: {}", e)),
    }
}

/// Channel-aware download + install. Re-runs the manifest check (so we always
/// install the latest announced version on the chosen channel) and streams
/// progress to the frontend via `update-progress` / `update-progress-finished`
/// events so the existing progress bar UI keeps working.
///
/// Returns the installed version on success.
#[tauri::command]
async fn install_update_with_channel(
    app: AppHandle,
    use_beta: bool,
) -> Result<String, String> {
    let updater = build_channel_updater(&app, use_beta)?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?
        .ok_or_else(|| "No update available on this channel".to_string())?;

    let version = update.version.clone();

    // Stream download progress to the frontend so the existing progress
    // bar in useUpdater.ts can render it.
    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    let on_chunk = move |chunk_len: usize, content_length: Option<u64>| {
        downloaded = downloaded.saturating_add(chunk_len as u64);
        let _ = progress_app.emit(
            "update-progress",
            serde_json::json!({
                "downloaded": downloaded,
                "total": content_length,
            }),
        );
    };
    let finish_app = app.clone();
    let on_finish = move || {
        let _ = finish_app.emit("update-progress-finished", ());
    };

    update
        .download_and_install(on_chunk, on_finish)
        .await
        .map_err(|e| format!("Download/install failed: {}", e))?;

    // Tauri's updater drops the freshly downloaded bundle into the app's
    // current location, but the new files carry the com.apple.quarantine
    // attribute. Without stripping it, the subsequent restart() spawns a
    // binary that macOS Gatekeeper silently blocks — which is why the
    // "Restart now" button has been doing nothing on every beta update.
    // Run xattr synchronously here so we know it's done BEFORE the
    // frontend calls relaunch().
    #[cfg(target_os = "macos")]
    {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(bundle) = exe
                .ancestors()
                .find(|p| p.extension().and_then(|s| s.to_str()) == Some("app"))
            {
                if bundle.starts_with("/Applications/") {
                    let _ = std::process::Command::new("xattr")
                        .args(["-dr", "com.apple.quarantine"])
                        .arg(bundle)
                        .output();
                }
            }
        }
    }

    Ok(version)
}

/// Called by the JS side after a silent background download + install
/// completes. The .app bundle on disk has already been replaced; this
/// command records the version that's waiting so the tray can surface a
/// blue dot + "Install update (vX.Y.Z)" menu item — the user's only
/// visible signal that an update is ready to take effect on next relaunch.
#[tauri::command]
fn mark_update_ready(app: AppHandle, version: String) -> Result<(), String> {
    crate::tray::set_pending_update(Some(version));
    crate::tray::refresh_tray(&app);
    Ok(())
}

/// Restart the app after a self-update completes. On macOS we use
/// `open -n -a <bundle>` instead of `app.restart()` because the standard
/// Tauri restart relies on an exec/spawn chain that has been silently
/// failing on freshly-installed beta builds — the old process dies but
/// the new one never appears (Sentry shows no error; users just see the
/// app vanish). LaunchServices via `open` handles signature/quarantine/
/// cache hand-off correctly.
///
/// `-n` forces a brand-new instance (without it, LaunchServices sees the
/// still-running current process and refuses to start a second copy).
/// We sleep briefly to let LaunchServices register the new app before
/// the current process exits.
///
/// Kept as a plain `pub fn` (not a `#[tauri::command]`) so other in-process
/// modules — e.g. the tray menu's "Install update" handler — can call it
/// directly without going through IPC. The `#[tauri::command]` wrapper
/// below just forwards to this so both call paths share the same code.
pub fn relaunch_app_via_launchservices(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let exe = std::env::current_exe()
            .map_err(|e| format!("current_exe failed: {}", e))?;
        let bundle = exe
            .ancestors()
            .find(|p| p.extension().and_then(|s| s.to_str()) == Some("app"))
            .ok_or_else(|| "Not running from a .app bundle".to_string())?
            .to_path_buf();

        let status = std::process::Command::new("open")
            .arg("-n")
            .arg("-a")
            .arg(&bundle)
            .status()
            .map_err(|e| format!("`open` command failed to spawn: {}", e))?;

        if !status.success() {
            return Err(format!(
                "`open -n -a {}` exited with status {}",
                bundle.display(),
                status
            ));
        }

        // Give LaunchServices time to register the new process before we die.
        std::thread::sleep(std::time::Duration::from_millis(300));
        app.exit(0);
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        // app.restart() triggers a process exit + respawn; on Windows it
        // works reliably because there's no Gatekeeper to negotiate.
        app.restart();
        // Unreachable in practice (restart() ends the process), but keeps
        // the type checker happy if Tauri's signature changes in future.
        #[allow(unreachable_code)]
        Ok(())
    }
}

#[tauri::command]
fn restart_app_post_update(app: AppHandle) -> Result<(), String> {
    relaunch_app_via_launchservices(app)
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

/// Open macOS System Settings directly on the Input Monitoring page so the
/// user can grant the permission without hunting through nested panes.
///
/// Hardcoded URL — we deliberately don't expose a generic `open_url` IPC
/// (would let any compromised JS open arbitrary system schemes). The
/// `x-apple.systempreferences:` scheme is the official macOS deep-link
/// protocol for jumping to a specific Privacy & Security pane.
#[tauri::command]
async fn open_input_monitoring_settings(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_microphone_settings(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_accessibility_settings(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    builder
        .manage(Mutex::new(AppState::default()))
        .manage(Mutex::new(RecordingContext::default()))
        .setup(move |app| {
            // Initialize license state (loads cached license + kicks off background refresh)
            licensing::init(app.handle());

            // Start the auto-trial on first launch (idempotent).
            usage::init();

            // Clean up stale audio backups (>24 hours old)
            transcription::backup::cleanup_stale_backups(app.handle());

            // Hide from dock — TTP is a tray-only app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Set up settings change listener for pill visibility updates
            tray::setup_settings_listener(app.handle());

            // Check accessibility permission (needed for paste simulation on macOS)
            #[cfg(target_os = "macos")]
            {
                let api_trusted = paste::check_accessibility();
                let actually_works = paste::probe_accessibility();

                if !api_trusted {
                    // Not trusted at all — prompt via the system dialog
                    paste::check_accessibility_with_prompt(true);
                    let _ = app.handle().emit("accessibility-missing", ());
                } else if !actually_works {
                    // Stale trust entry (common after app update) — reset and re-prompt
                    eprintln!(
                        "[TTP] Accessibility trust is stale after update. Resetting TCC entry."
                    );
                    if let Err(e) = paste::reset_accessibility_tcc() {
                        eprintln!("[TTP] Failed to reset TCC: {}", e);
                    }
                    // Small delay then re-prompt
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    paste::check_accessibility_with_prompt(true);
                    let _ = app.handle().emit("accessibility-missing", ());
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app.notification()
                        .builder()
                        .title(crate::i18n::tr("notification.accessibilityRegrantTitle"))
                        .body(crate::i18n::tr("notification.accessibilityRegrantBody"))
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

            // Best-effort: clear the macOS quarantine xattr after a self-update so
            // Gatekeeper doesn't re-prompt the user. Only runs when installed in
            // /Applications (i.e. real users, never dev mode), and silently no-ops
            // if `xattr` isn't on PATH.
            //
            // We walk up from the current executable to find the actual .app
            // bundle path — previously this hardcoded "/Applications/TTP.app"
            // but the product name is "TTP by AmirKS", so the hardcoded path
            // never matched and the xattr clear was silently a no-op.
            #[cfg(target_os = "macos")]
            {
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(bundle) = exe
                        .ancestors()
                        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("app"))
                    {
                        if bundle.starts_with("/Applications/") {
                            let bundle = bundle.to_path_buf();
                            // Use Tauri's runtime-attached worker pool rather
                            // than bare std::thread::spawn. The xattr call
                            // itself is sync, but spawning on Tauri's runtime
                            // means any panic/instrumentation lands inside the
                            // runtime context (sentry breadcrumbs, panic hooks)
                            // rather than on a detached thread that can't
                            // unwind cleanly when the app exits.
                            tauri::async_runtime::spawn_blocking(move || {
                                let _ = std::process::Command::new("xattr")
                                    .args(["-dr", "com.apple.quarantine"])
                                    .arg(&bundle)
                                    .output();
                            });
                        }
                    }
                }
            }

            // Load persisted hands_free_mode from settings
            let hands_free_mode = settings::get_settings().hands_free_mode;
            if let Some(state) = app.try_state::<Mutex<AppState>>() {
                if let Ok(mut app_state) = state.try_lock() {
                    app_state.hands_free_mode = hands_free_mode;
                }
            }

            // Respect the `hide_pill_when_inactive` setting at startup — without this,
            // the pill always reappeared after a relaunch even when the user had hidden it.
            if tray::should_show_pill(app.handle()) {
                tray::show_pill(app.handle());
            }

            // Check if this is the first launch
            let is_first = permissions::is_first_launch();
            if is_first {
                // Show onboarding window (permission check flow)
                let _ = onboarding::show_onboarding(app.handle().clone());
            } else {
                // Not first launch - check if Groq API key exists, show setup window if not
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
            validate_groq_api_key,
            get_recordings_dir,
            process_audio,
            get_settings,
            set_settings,
            reset_settings,
            open_settings_window,
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
            check_input_monitoring_permission,
            request_input_monitoring_permission,
            open_input_monitoring_settings,
            open_microphone_settings,
            open_accessibility_settings,
            reset_to_idle,
            restart_app_post_update,
            check_for_updates_with_channel,
            install_update_with_channel,
            mark_update_ready,
            check_microphone_permission,
            is_first_launch_cmd,
            mark_first_launch_complete_cmd,
            permissions::request_microphone_permission,
            check_accessibility_permission,
            request_accessibility_permission,
            reset_accessibility_permission,
            show_onboarding,
            close_onboarding,
            check_whats_new,
            dismiss_whats_new,
            activate_license,
            deactivate_license,
            validate_license,
            is_pro,
            get_license_info,
            get_usage_stats,
            get_analytics_summary,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |handler, event| {
            match event {
                tauri::RunEvent::Ready { .. } => {
                    telemetry::analytics::track(handler, "app_started", None);
                }
                _ => {}
            }
        });
}
