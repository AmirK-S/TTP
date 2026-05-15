// TTP - Talk To Paste
// Analytics module — kept as a no-op shim after Aptabase was removed.
//
// The `tauri-plugin-aptabase` crate was the source of a recurring "no
// reactor running" panic (TTP-A/B/D/E): its background event-flush task
// called `reqwest` → `tokio::time::sleep` → `tokio::runtime::Handle::current`
// from a thread that wasn't attached to a Tokio runtime, panicking the
// whole app. Forking or wrapping the plugin would have been a maintenance
// burden, so the cleanest durable fix was to remove the dependency
// entirely. Sentry continues to capture crashes and errors — the only
// thing we lose is product analytics nobody was looking at anyway.
//
// This module is preserved as a stub so existing `track()` call sites
// across the codebase keep compiling without per-call cleanup. Future
// removal of these calls is harmless cleanup, not a requirement.

/// No-op analytics event tracker. Signature preserved so callers don't
/// need to change; the body is empty so no HTTP, no Tokio runtime, no
/// background task. Safe to call from any thread, any runtime context.
pub fn track(_app: &tauri::AppHandle, _event: &str, _props: Option<serde_json::Value>) {}
