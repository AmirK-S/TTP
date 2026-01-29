// TTP - Talk To Paste
// Main Tauri application entry point

mod credentials;
mod paste;
mod recording;
mod shortcuts;
mod sounds;
mod state;
mod transcription;
mod tray;

use credentials::{delete_api_key, get_api_key, has_api_key, set_api_key};
use recording::{get_recordings_dir, RecordingContext};
use state::AppState;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_keyring::KeyringExt;

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

            // Check if API key exists, show setup window if not
            let has_key = app
                .keyring()
                .get_password(SERVICE_NAME, API_KEY_USER)
                .map(|k| k.is_some())
                .unwrap_or(false);

            if !has_key {
                // Show setup window for first-run experience
                if let Some(window) = app.get_webview_window("setup") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    println!("First run: showing API key setup window");
                }
            } else {
                println!("API key found in keychain");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_api_key,
            set_api_key,
            has_api_key,
            delete_api_key,
            get_recordings_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
