// TTP - Talk To Paste
// Main Tauri application entry point

mod credentials;
mod dictionary;
mod history;
mod paste;
mod recording;
mod settings;
mod shortcuts;
mod sounds;
mod state;
mod transcription;
mod tray;

use credentials::{
    delete_api_key, delete_gladia_api_key, delete_groq_api_key, get_api_key, get_gladia_api_key,
    get_groq_api_key, has_api_key, has_gladia_api_key, has_groq_api_key, set_api_key,
    set_gladia_api_key, set_groq_api_key,
};
use dictionary::{clear_dictionary, delete_dictionary_entry, get_dictionary};
use history::{clear_history, get_history};
use recording::{get_recordings_dir, RecordingContext};
use settings::{get_settings, reset_settings, set_settings};
use transcription::process_audio;
use state::AppState;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_keyring::KeyringExt;

/// Tauri command to update the global shortcut at runtime
#[tauri::command]
fn update_shortcut_cmd(app: AppHandle, shortcut: String) -> Result<(), String> {
    shortcuts::update_shortcut(&app, &shortcut)
}

/// Tauri command to reset state to Idle (used when skipping short recordings)
#[tauri::command]
fn reset_to_idle(app: AppHandle) {
    println!("[reset_to_idle] Called - recording was too short or mic failed");
    if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = state.try_lock() {
            guard.set_state(state::RecordingState::Idle, &app);
            // Also reset tray icon
            tray::set_recording_icon(&app, false);
        }
    }
}

/// Debug command to log messages from frontend to terminal
#[tauri::command]
fn debug_log(message: String) {
    println!("[Frontend] {}", message);
}

const SERVICE_NAME: &str = "TTP";
const API_KEY_USER: &str = "openai-api-key";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_mic_recorder::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Mutex::new(AppState::default()))
        .manage(Mutex::new(RecordingContext::default()))
        .setup(|app| {
            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Set up global keyboard shortcuts
            shortcuts::setup_shortcuts(app.handle())?;

            // Position floating bar above the dock
            if let Some(window) = app.get_webview_window("floating-bar") {
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let screen_size = monitor.size();
                    let window_width = 100.0;
                    let window_height = 32.0;
                    let dock_offset = 220.0;

                    let x = (screen_size.width as f64 / 2.0) - (window_width / 2.0);
                    let y = screen_size.height as f64 - dock_offset - window_height;

                    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                        x: x as i32,
                        y: y as i32,
                    }));
                }
            }

            // Check if API key exists (env var first, then keychain), show setup window if not
            let has_key = if std::env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false) {
                println!("API key found in environment variable");
                true
            } else {
                let from_keyring = app
                    .keyring()
                    .get_password(SERVICE_NAME, API_KEY_USER)
                    .map(|k| k.is_some())
                    .unwrap_or(false);
                if from_keyring {
                    println!("API key found in keychain");
                }
                from_keyring
            };

            if !has_key {
                // Show setup window for first-run experience
                if let Some(window) = app.get_webview_window("setup") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    println!("First run: showing API key setup window");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_api_key,
            set_api_key,
            has_api_key,
            delete_api_key,
            get_groq_api_key,
            set_groq_api_key,
            has_groq_api_key,
            delete_groq_api_key,
            get_gladia_api_key,
            set_gladia_api_key,
            has_gladia_api_key,
            delete_gladia_api_key,
            get_recordings_dir,
            process_audio,
            get_settings,
            set_settings,
            reset_settings,
            get_dictionary,
            delete_dictionary_entry,
            clear_dictionary,
            get_history,
            clear_history,
            update_shortcut_cmd,
            reset_to_idle,
            debug_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
