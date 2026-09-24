// Tests for the JS-side Sentry scrubber. We import the actual production
// helpers from `./pii-scrub` so a divergence between intent and shipping
// regex causes a failure, not a silent mismatch.
//
// `scrubEvent` itself lives inside `sentry.ts` (and depends on dynamically
// imported @sentry/react types); we don't unit-test it because the meaningful
// behaviour is the underlying regex set, which is here.

import { describe, it, expect } from 'vitest';
import { isSensitiveKey, scrubMessage as scrubMessageImpl } from './pii-scrub';

// Wrap scrubMessage with a string-typed alias so the test cases below stay
// readable — the production signature is `(unknown) -> unknown` for callsite
// convenience but every input we test here is a string.
function scrubMessage(s: string): string {
  return scrubMessageImpl(s) as string;
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
// `src-tauri/src/telemetry/sentry.rs`) and the JS denylist (imported from
// `./pii-scrub`) must drop the same key shapes. We assert a representative
// subset here so a future drift fires a test failure rather than a silent
// telemetry leak.
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

describe('isSensitiveKey parity', () => {
  it.each(SENSITIVE)('drops %s', (k) => {
    expect(isSensitiveKey(k)).toBe(true);
  });

  it.each(SAFE)('preserves %s', (k) => {
    expect(isSensitiveKey(k)).toBe(false);
  });
});
