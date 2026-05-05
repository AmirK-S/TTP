// TTP - Talk To Paste
// Analytics wrapper — calls the tauri-plugin-aptabase Rust command directly.
// We bypass @aptabase/tauri because it ships a bundled @tauri-apps/api v1
// that calls window.__TAURI_IPC__() — undefined under Tauri 2 (TTP-8/TTP-6).

import { invoke } from '@tauri-apps/api/core';

export function trackEvent(name: string, props?: Record<string, string | number>) {
  // Plugin is only registered when telemetry is opted in — on opt-out the
  // invoke rejects and we silently drop the event.
  invoke('plugin:aptabase|track_event', { name, props }).catch(() => {});
}
