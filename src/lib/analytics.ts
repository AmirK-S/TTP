// TTP - Talk To Paste
// Safe analytics wrapper - silently no-ops when Aptabase plugin is not registered (telemetry OFF)

import { trackEvent as aptabaseTrackEvent } from "@aptabase/tauri";

/**
 * Track an analytics event. Silently no-ops when telemetry is disabled
 * (Aptabase plugin not registered).
 */
export function trackEvent(name: string, props?: Record<string, string | number>) {
  try {
    aptabaseTrackEvent(name, props);
  } catch {
    // Silently ignore -- plugin not registered when telemetry is OFF
  }
}
