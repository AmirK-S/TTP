import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// IMPORTANT: vi.mock hoists. We mock '@tauri-apps/api/core' BEFORE importing
// safeInvoke so the wrapper sees our mock, not the real one. The setup file
// also installs a global invoke mock, but safeInvoke is wired against the
// pre-import-time mock so the per-file mock here wins.
const tauriInvokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => tauriInvokeMock(...args),
}));

import { safeInvoke } from './safeInvoke';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

describe('safeInvoke', () => {
  beforeEach(() => {
    tauriInvokeMock.mockReset();
    // Reset the module-level cache by re-importing? Not trivial without
    // module reset hooks. Instead we exercise the "already ready" path by
    // pre-setting the bridge sentinel and exercise the wait path by deleting
    // it before the test.
  });

  afterEach(() => {
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it('forwards cmd + args to the real invoke once the bridge is ready', async () => {
    window.__TAURI_INTERNALS__ = {};
    tauriInvokeMock.mockResolvedValueOnce('ok');

    const result = await safeInvoke<string>('get_settings', { foo: 1 });

    expect(result).toBe('ok');
    expect(tauriInvokeMock).toHaveBeenCalledTimes(1);
    expect(tauriInvokeMock).toHaveBeenCalledWith('get_settings', { foo: 1 });
  });

  it('propagates the Promise type so callers can await typed results', async () => {
    window.__TAURI_INTERNALS__ = {};
    tauriInvokeMock.mockResolvedValueOnce({ telemetry_enabled: true });

    const result = await safeInvoke<{ telemetry_enabled: boolean }>('get_settings');

    expect(result.telemetry_enabled).toBe(true);
  });

  it('rejects with the underlying invoke error', async () => {
    window.__TAURI_INTERNALS__ = {};
    tauriInvokeMock.mockRejectedValueOnce(new Error('command failed'));

    await expect(safeInvoke('broken_command')).rejects.toThrow('command failed');
  });
});
