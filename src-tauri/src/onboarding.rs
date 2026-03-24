// TTP - Talk To Paste
// Onboarding window management module

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::command;
use crate::credentials;

/// Show the onboarding window - triggers native macOS permission dialog
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
    .inner_size(480.0, 600.0)
    .resizable(false)
    .center()
    .always_on_top(true)
    .build()
    .map_err(|e| format!("Failed to create onboarding window: {}", e))?;
    
    Ok(())
}

/// Close onboarding and mark first launch complete (no setup window needed)
#[command]
pub fn close_onboarding(app: AppHandle) -> Result<(), String> {
    // Close onboarding window
    if let Some(window) = app.get_webview_window("onboarding") {
        let _ = window.close();
    }

    // Mark first launch complete
    let _ = crate::permissions::mark_first_launch_complete();

    Ok(())
}
