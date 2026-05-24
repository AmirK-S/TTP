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

/// Close onboarding, mark first launch complete, suppress the "What's New"
/// popup for this version (the wizard already told them what's new), and
/// surface Settings so the user can see their freshly-applied preferences.
#[command]
pub fn close_onboarding(app: AppHandle) -> Result<(), String> {
    // Close onboarding window
    if let Some(window) = app.get_webview_window("onboarding") {
        let _ = window.close();
    }

    // Mark first launch complete
    let _ = crate::permissions::mark_first_launch_complete();

    // Suppress the WhatsNew modal for the version we just onboarded the user
    // to. They've just clicked through a five-step wizard for this exact
    // build, a "here's what's new in vX.Y.Z" follow-up modal is redundant.
    // WhatsNew will fire normally on the next auto-update.
    let _ = crate::whatsnew::dismiss_whats_new();

    // Surface Settings so the user can see (and tweak) what they just
    // configured. Settings.tsx calls `loadSettings` on mount, so it reads
    // the freshly-persisted state and the toggles correctly reflect the
    // wizard choices — no restart needed, no stale-store overwrite race.
    if let Some(settings) = app.get_webview_window("settings") {
        let _ = settings.show();
        let _ = settings.set_focus();
    }

    Ok(())
}
