// TTP - Talk To Paste
// Shared PII scrubbing primitives for Sentry events and breadcrumbs.
//
// Mirrors the regex set in `src-tauri/src/telemetry/sentry.rs`. When you
// bump one side, bump the other. The duplication is intentional: the
// scrubber runs both in the JS process (for renderer-originated events)
// and in the Rust process (for backend events + native crash payloads).
// Drift between the two would mean a Sentry event that should have been
// scrubbed leaks because of which side it originated from.

/**
 * Keys whose values may contain credentials, paths, or transcript content.
 * Substring matches are case-insensitive.
 *
 * The "key" substring is intentional but has known false positives like
 * "keystroke" / "keypress". We accept that trade-off: dropping a debug
 * field is cheaper than leaking the Groq API key when an extra field is
 * misnamed.
 */
export const SENSITIVE_KEY_SUBSTRINGS = [
  'api_key',
  'apikey',
  'key',
  'token',
  'password',
  'license',
  'secret',
  'path',
  'text',
  'transcription',
] as const;

export function isSensitiveKey(key: string): boolean {
  const lower = key.toLowerCase();
  return SENSITIVE_KEY_SUBSTRINGS.some((s) => lower.includes(s));
}

// Regex patterns. Each is anchored to the shape of the secret/PII it
// removes, so a benign substring resembling one doesn't trigger.
export const API_KEY_RE = /\b(?:sk-|gsk_)[A-Za-z0-9_-]{16,}/g;
export const EMAIL_RE = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;
export const FILE_PATH_RE =
  /(?:\/Users\/[^\s"'`]+|\/private\/var\/[^\s"'`]+|[A-Z]:\\[^\s"'`]+)/g;

/**
 * Scrub a free-form string of API keys, emails, and full filesystem paths.
 * Non-string inputs are returned untouched so callers don't have to
 * type-narrow before invoking.
 */
export function scrubMessage(s: unknown): unknown {
  if (typeof s !== 'string') return s;
  return s
    .replace(API_KEY_RE, '[REDACTED_API_KEY]')
    .replace(EMAIL_RE, '[REDACTED_EMAIL]')
    .replace(FILE_PATH_RE, '[REDACTED_PATH]');
}
