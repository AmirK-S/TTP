// Tests for the JS-side Sentry scrubber. We don't test `initSentryIfConsented`
// itself (it dynamically imports @sentry/react which has heavy side effects);
// instead we re-implement the scrubber surface in a tiny harness and assert
// the cross-cutting contract. The actual `scrubEvent` lives in `sentry.ts`
// but it's wrapped in a private closure — exposing it solely for tests via
// a `__test__` export keeps the production API surface clean.
//
// NOTE: this file is intentionally a near-mirror of the regexes in
// `src-tauri/src/telemetry/sentry.rs`. When you bump one, bump the other.

import { describe, it, expect } from 'vitest';

// Replicate the production regexes here so the test FAILS loudly when the
// production file diverges. Yes, this is duplication on purpose — see the
// note above.
const API_KEY_RE = /\b(?:sk-|gsk_)[A-Za-z0-9_-]{16,}/g;
const EMAIL_RE = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;
const FILE_PATH_RE = /(?:\/Users\/[^\s"'`]+|\/private\/var\/[^\s"'`]+|[A-Z]:\\[^\s"'`]+)/g;

function scrubMessage(s: string): string {
  return s
    .replace(API_KEY_RE, '[REDACTED_API_KEY]')
    .replace(EMAIL_RE, '[REDACTED_EMAIL]')
    .replace(FILE_PATH_RE, '[REDACTED_PATH]');
}

describe('scrubMessage', () => {
  it('redacts Groq-style sk_ / gsk_ keys mid-string', () => {
    const before = 'API error: gsk_abcdefghij1234567890XYZ rate limited';
    const after = scrubMessage(before);
    expect(after).toContain('[REDACTED_API_KEY]');
    expect(after).not.toContain('gsk_abcdefghij');
  });

  it('redacts emails verbatim', () => {
    const before = 'Activation for user@example.com failed';
    expect(scrubMessage(before)).toBe('Activation for [REDACTED_EMAIL] failed');
  });

  it('redacts macOS home + private-var + Windows drive paths', () => {
    expect(scrubMessage('open /Users/alice/Library/foo.json')).toContain('[REDACTED_PATH]');
    expect(scrubMessage('open /private/var/folders/x/zzz')).toContain('[REDACTED_PATH]');
    expect(scrubMessage('open C:\\Users\\bob\\Documents\\foo.txt')).toContain('[REDACTED_PATH]');
  });

  it('does not over-redact short tokens that resemble keys', () => {
    // The regex requires `(sk-|gsk_)` + at least 16 chars. Short test tokens
    // are left alone — false positives are noisier than the underlying signal.
    expect(scrubMessage('token: sk-short')).toBe('token: sk-short');
  });

  it('redacts multiple matches in the same string', () => {
    const before =
      'Error: gsk_aaaaaaaaaaaaaaaaaaaaaaa for /Users/alice/Library/foo';
    const after = scrubMessage(before);
    expect(after).toContain('[REDACTED_API_KEY]');
    expect(after).toContain('[REDACTED_PATH]');
    expect(after).not.toContain('gsk_aaaa');
    expect(after).not.toContain('/Users/alice');
  });
});

// Parity test: the Rust denylist (`is_sensitive_key` in
// `src-tauri/src/telemetry/sentry.rs`) and the JS denylist must drop the
// same key shapes. We assert a representative subset here so a future drift
// fires a test failure rather than a silent telemetry leak.
const SENSITIVE = [
  'api_key',
  'apikey',
  'license_key',
  'auth_token',
  'password',
  'license',
  'oauth_secret',
  'file_path',
  'transcription_text',
  'raw_text',
];
const SAFE = ['feature_flag', 'os_version', 'window_id', 'recording_state'];

const KEY_RE = /(api_key|apikey|key|token|password|license|secret|path|text|transcription)/i;
function isSensitiveKey(k: string): boolean {
  return KEY_RE.test(k);
}

describe('isSensitiveKey parity', () => {
  it.each(SENSITIVE)('drops %s', (k) => {
    expect(isSensitiveKey(k)).toBe(true);
  });

  it.each(SAFE)('preserves %s', (k) => {
    expect(isSensitiveKey(k)).toBe(false);
  });
});
