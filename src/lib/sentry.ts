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
//
// IMPORTANT: `@sentry/react` is NEVER statically imported here. It ships
// ~80KB and would land in every window's bundle — including the pill, which
// must stay lean. Instead we dynamically import the SDK inside
// `initSentryIfConsented`, so only consented-and-eligible windows pay the
// cost, and the pill never pulls it in at all.

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
 *
 * Typed as `any` since we no longer import the Sentry types statically;
 * the runtime shape matches `Sentry.ErrorEvent`.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function scrubEvent(event: any): any {
  if (event?.request?.cookies) {
    delete event.request.cookies;
  }
  if (event?.user) {
    delete event.user.email;
    delete event.user.ip_address;
  }
  if (event?.extra) {
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
 *
 * Dynamically imports `@sentry/react` so the SDK only lands in the bundle
 * of windows that actually call this (Settings/Onboarding). The pill
 * window never calls it, so it never pays the ~80KB cost.
 */
export async function initSentryIfConsented(): Promise<void> {
  if (initStarted) return;
  initStarted = true;

  try {
    const settings = await safeInvoke<SettingsSnapshot>('get_settings');
    if (!settings?.telemetry_enabled) return;

    const Sentry = await import('@sentry/react');

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

/**
 * Best-effort capture for errors caught by our in-house ErrorBoundary.
 * Lazily pulls in `@sentry/react` only if telemetry was actually initialised,
 * keeping the cost off the cold-start path. Never throws.
 */
export async function captureExceptionIfActive(error: unknown): Promise<void> {
  if (!initStarted) return;
  try {
    const Sentry = await import('@sentry/react');
    Sentry.captureException(error);
  } catch {
    // Telemetry failures must never break the app.
  }
}
