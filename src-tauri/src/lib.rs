// TTP - Talk To Paste
// Main Tauri application entry point

mod credentials;
mod shortcuts;
mod sounds;
mod state;
mod tray;

use credentials::{delete_api_key, get_api_key, has_api_key, set_api_key};
use state::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_keyring::init())
        .manage(Mutex::new(AppState::default()))
        .setup(|app| {
            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Set up global keyboard shortcuts
            shortcuts::setup_shortcuts(app.handle())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_api_key,
            set_api_key,
            has_api_key,
            delete_api_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
