// TTP - Talk To Paste
// Analytics wrapper — no-op after Aptabase was removed in v2.0.4.
//
// The `tauri-plugin-aptabase` plugin was the source of a recurring panic
// ("no reactor running") that crashed the app. Cleanest durable fix was
// to drop the plugin entirely; Sentry still captures crashes. This
// module stays as a no-op shim so existing call sites compile without
// per-call cleanup.

export function trackEvent(_name: string, _props?: Record<string, string | number>) {
  // intentionally empty
}
