// TTP - Talk To Paste
// System tray setup and management

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TTP", true, None::<&str>)?;

    // Build context menu
    let menu = Menu::with_items(app, &[&settings, &quit])?;

    // Use default icon (configured in tauri.conf.json trayIcon)
    let icon = app.default_window_icon().cloned().ok_or("No default icon")?;

    // Build tray icon with ID for later reference
    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false) // Right-click for menu
        .tooltip("TTP - Talk To Paste")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "settings" => {
                // Open settings window when implemented
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Update the tray icon to reflect recording state
/// Note: For icon changes, we'll use the default icon for now
/// Custom icons will be added when proper icon loading is implemented
pub fn set_recording_icon(app: &AppHandle, _recording: bool) {
    // TODO: Implement custom icon switching when icons are embedded as resources
    // For now, tray icon remains static (configured in tauri.conf.json)
    if let Some(tray) = app.tray_by_id("main") {
        if let Some(icon) = app.default_window_icon().cloned() {
            let _ = tray.set_icon(Some(icon));
        }
    }
}
