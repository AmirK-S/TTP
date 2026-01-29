// TTP - Talk To Paste
// Main Tauri application entry point

mod credentials;
mod recording;
mod shortcuts;
mod sounds;
mod state;
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
        .manage(Mutex::new(AppState::default()))
        .manage(Mutex::new(RecordingContext::default()))
        .setup(|app| {
            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Set up global keyboard shortcuts
            shortcuts::setup_shortcuts(app.handle())?;

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
