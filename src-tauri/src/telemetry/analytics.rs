// TTP - Talk To Paste
// Analytics module - Aptabase app key for privacy-first usage analytics

/// Aptabase app key (public-facing, identifies the app for event routing).
/// EU region key routes to eu.aptabase.com automatically.
pub const APTABASE_APP_KEY: &str = "A-EU-6676689196";

/// Marker struct stored as Tauri managed state when Aptabase plugin is registered.
/// Check with `app.try_state::<TelemetryActive>().is_some()` before calling track_event.
pub struct TelemetryActive;

/// Safe wrapper to track an analytics event. No-ops when telemetry is disabled.
pub fn track(app: &tauri::AppHandle, event: &str, props: Option<serde_json::Value>) {
    use tauri::Manager;
    if app.try_state::<TelemetryActive>().is_some() {
        use tauri_plugin_aptabase::EventTracker;
        let _ = app.track_event(event, props);
    }
}
