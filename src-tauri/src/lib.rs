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

/// Tauri command to update the global shortcut at runtime
#[tauri::command]
fn update_shortcut_cmd(app: AppHandle, shortcut: String) -> Result<(), String> {
    shortcuts::update_shortcut(&app, &shortcut)
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    println!("[Shortcut] Event: {:?}", event.state());
                    shortcuts::handle_shortcut_event_public(app, event.state());
                })
                .build(),
        )
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_mic_recorder::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Mutex::new(AppState::default()))
        .manage(Mutex::new(RecordingContext::default()))
        .setup(|app| {
            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Check accessibility permission (needed for paste simulation on macOS)
            #[cfg(target_os = "macos")]
            {
                if !paste::check_accessibility() {
                    println!("Accessibility permission not granted");
                    let _ = std::process::Command::new("open")
                        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        .spawn();
                } else {
                    println!("Accessibility permission granted");
                }
            }

            // Set up global keyboard shortcuts
            shortcuts::setup_shortcuts(app.handle())?;

            // Show pill window (always visible)
            tray::show_pill(app.handle());

            // Check if API key exists, show setup window if not
            let has_key = std::env::var("OPENAI_API_KEY")
                .map(|k| !k.is_empty())
                .unwrap_or(false);

            if !has_key {
                // Show setup window for first-run experience
                if let Some(window) = app.get_webview_window("setup") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    println!("First run: showing API key setup window");
                }
            } else {
                println!("API key found in environment");
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
            reset_to_idle
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
