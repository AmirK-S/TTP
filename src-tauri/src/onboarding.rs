// TTP - Talk To Paste
// Onboarding window management module

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::command;

/// Show the onboarding window
/// Creates a new centered window for the onboarding flow
#[command]
pub fn show_onboarding(app: AppHandle) -> Result<(), String> {
    // Check if onboarding window already exists
    if let Some(window) = app.get_webview_window("onboarding") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    
    // Create new onboarding window
    let _window = WebviewWindowBuilder::new(
        &app,
        "onboarding",
        WebviewUrl::App("onboarding.html".into()),
    )
    .title("Welcome to Talk To Paste")
    .inner_size(500.0, 400.0)
    .resizable(false)
    .center()
    .always_on_top(true)
    .build()
    .map_err(|e| format!("Failed to create onboarding window: {}", e))?;
    
    Ok(())
}

/// Close the onboarding window
#[command]
pub fn close_onboarding(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("onboarding") {
        window.close().map_err(|e| format!("Failed to close onboarding window: {}", e))?;
    }
    Ok(())
}
