// TTP - Talk To Paste
// Frontend Sentry init - mirrors the Rust SDK's consent gate and PII scrubbing.
//
// The Rust side (src-tauri/src/lib.rs:139-172) only passes a DSN to
// sentry::init() when settings.telemetry_enabled is true; otherwise the
// client is fully disabled and no events leave the device. We mirror that
// here: nothing is initialised until safeInvoke('get_settings') confirms
// the user has opted in.
//
// DSN is the same one used on the Rust side; we duplicate it here rather
// than expose another IPC command. Release tag is baked in via Vite
// (see vite.config.ts) so JS events line up with Rust events on the same
// Sentry release.

import * as Sentry from '@sentry/react';
import { safeInvoke } from './safeInvoke';

// Same DSN as src-tauri/src/telemetry/consent.rs. Public, write-only.
const SENTRY_DSN =
  'https://e74857f6958a0049cf61eefbdc40d3e6@o4510885403033600.ingest.de.sentry.io/4510885412274256';

// Subset of the Settings struct from src-tauri/src/settings/store.rs.
type SettingsSnapshot = { telemetry_enabled?: boolean };

/** Keys whose values may contain credentials or other secrets. */
function isSensitiveKey(key: string): boolean {
  const lower = key.toLowerCase();
  return (
    lower.includes('api_key') ||
    lower.includes('apikey') ||
    lower.includes('token') ||
    lower.includes('password') ||
    lower.includes('license') ||
    lower.includes('secret')
  );
}

/**
 * Strip obvious PII before an event leaves the device. Mirrors the scope
 * of scrub_event_pii() in src-tauri/src/telemetry/sentry.rs (cookies,
 * email/IP on user, sensitive `extra` keys). Regex-level scrubbing of
 * file paths and Groq keys inside exception messages is left to the
 * Rust side — JS exceptions don't usually carry those.
 */
function scrubEvent(event: Sentry.ErrorEvent): Sentry.ErrorEvent | null {
  if (event.request?.cookies) {
    delete event.request.cookies;
  }
  if (event.user) {
    delete event.user.email;
    delete event.user.ip_address;
  }
  if (event.extra) {
    for (const key of Object.keys(event.extra)) {
      if (isSensitiveKey(key)) {
        delete event.extra[key];
      }
    }
  }
  return event;
}

let initStarted = false;

/**
 * Initialise Sentry only if the user has opted into telemetry.
 *
 * Safe to call multiple times — guarded by `initStarted` and Sentry's
 * own internal hub guard. Never throws: any failure is logged and
 * swallowed, since telemetry must never break the app.
 */
export async function initSentryIfConsented(): Promise<void> {
  if (initStarted) return;
  initStarted = true;

  try {
    const settings = await safeInvoke<SettingsSnapshot>('get_settings');
    if (!settings?.telemetry_enabled) return;

    Sentry.init({
      dsn: SENTRY_DSN,
      release: import.meta.env.VITE_APP_VERSION,
      environment: import.meta.env.DEV ? 'development' : 'production',
      // Never auto-attach IPs / cookies. PII is stripped explicitly in
      // beforeSend below as a second line of defence.
      sendDefaultPii: false,
      // Out of scope: tracing & replay (kept off to stay near the
      // ~30-40KB minified budget, see task brief).
      integrations: [],
      beforeSend: scrubEvent,
    });
  } catch (err) {
    // Don't let a telemetry failure crash the window.
    // eslint-disable-next-line no-console
    console.warn('[sentry] init skipped:', err);
  }
}

/** Re-export the SDK's ErrorBoundary so callers don't import @sentry/react directly. */
export const ErrorBoundary = Sentry.ErrorBoundary;
