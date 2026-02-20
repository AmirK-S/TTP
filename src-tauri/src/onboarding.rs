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

use crate::credentials;

/// Close the onboarding window and show setup if needed
#[command]
pub fn close_onboarding(app: AppHandle) -> Result<(), String> {
    // Close the onboarding window
    if let Some(window) = app.get_webview_window("onboarding") {
        window.close().map_err(|e| format!("Failed to close onboarding window: {}", e))?;
    }
    
    // Mark first launch as complete
    crate::permissions::mark_first_launch_complete();
    
    // Check if API key exists, show setup if not
    let has_groq = credentials::get_groq_api_key_internal(&app)
        .map(|k| k.is_some())
        .unwrap_or(false);
    
    if !has_groq {
        // Show setup window
        if let Some(window) = app.get_webview_window("setup") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    
    Ok(())
}
