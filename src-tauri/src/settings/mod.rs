// TTP - Talk To Paste
// Settings module - manages application settings and persistence

pub mod store;

pub use store::{get_settings, reset_settings, set_settings, Settings};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::command;

/// Open or focus the settings window
/// If the window already exists, show it and focus it (foreground)
/// If not, create a new settings window
#[command]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // Check if settings window already exists
    if let Some(window) = app.get_webview_window("settings") {
        // Window exists - show it and bring to foreground
        // Critical order: show() BEFORE set_focus() per prior decision
        window.show().map_err(|e| format!("Failed to show settings window: {}", e))?;
        window.set_focus().map_err(|e| format!("Failed to focus settings window: {}", e))?;
        return Ok(());
    }

    // Create new settings window
    let _window = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html".into()),
    )
    .title("TTP by AmirKS — Settings")
    .inner_size(500.0, 600.0)
    .resizable(true)
    .visible(false)
    .center()
    .decorations(true)
    .build()
    .map_err(|e| format!("Failed to create settings window: {}", e))?;

    // Show immediately after creation (before focus)
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| format!("Failed to show new settings window: {}", e))?;
        window.set_focus().map_err(|e| format!("Failed to focus new settings window: {}", e))?;
    }

    Ok(())
}
