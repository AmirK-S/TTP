import { describe, it, expect, vi } from 'vitest';
import { translateRustMessage } from './translateRustMessage';

describe('translateRustMessage', () => {
  it('resolves error.* keys via t()', () => {
    const t = vi.fn((k: string) => `T(${k})`);
    expect(translateRustMessage(t, 'error.no_speech')).toBe('T(error.no_speech)');
    expect(t).toHaveBeenCalledWith('error.no_speech', undefined);
  });

  it('resolves progress.* and permission.* keys via t()', () => {
    const t = vi.fn((k: string) => `T(${k})`);
    expect(translateRustMessage(t, 'progress.transcribing')).toBe('T(progress.transcribing)');
    expect(translateRustMessage(t, 'permission.input_monitoring_required')).toBe(
      'T(permission.input_monitoring_required)',
    );
  });

  it('passes interpolation params through to t()', () => {
    const t = vi.fn((k: string, p?: Record<string, string | number>) =>
      p ? `${k}(${JSON.stringify(p)})` : k,
    );
    const out = translateRustMessage(t, 'error.api_generic', { status: 503, body: 'oops' });
    expect(t).toHaveBeenCalledWith('error.api_generic', { status: 503, body: 'oops' });
    expect(out).toBe('error.api_generic({"status":503,"body":"oops"})');
  });

  it('returns the literal message when no namespace prefix matches', () => {
    const t = vi.fn(() => 'NEVER');
    // A literal English fallback from an older Rust build.
    expect(translateRustMessage(t, 'Recording is already in progress')).toBe(
      'Recording is already in progress',
    );
    expect(t).not.toHaveBeenCalled();
  });

  it('returns the literal message when the key has no dot', () => {
    const t = vi.fn();
    expect(translateRustMessage(t, 'transcribing')).toBe('transcribing');
    expect(t).not.toHaveBeenCalled();
  });

  it('returns "" for empty input without calling t()', () => {
    const t = vi.fn();
    expect(translateRustMessage(t, '')).toBe('');
    expect(t).not.toHaveBeenCalled();
  });

  it('does not translate unknown namespaced keys (e.g. random.foo)', () => {
    const t = vi.fn(() => 'NEVER');
    expect(translateRustMessage(t, 'random.foo')).toBe('random.foo');
    expect(t).not.toHaveBeenCalled();
  });
});
