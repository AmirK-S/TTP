import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

// ── Mock setup ────────────────────────────────────────────────────────────
// Mock the Tauri / plugin surfaces useUpdater touches so the hook can run
// in node with happy-dom and we can script each IPC outcome per test.
const mockInvokeFn = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvokeFn(cmd, args),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}));
vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn(async () => '3.0.4'),
}));
vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(async () => {}),
}));
vi.mock('../lib/analytics', () => ({
  trackEvent: vi.fn(),
}));

// useRecordingState reads from a Tauri event. We mock the hook itself so
// each test can pin the state without firing simulated events.
const mockRecordingState = vi.fn(() => 'Idle' as 'Idle' | 'Recording' | 'Processing');
vi.mock('./useRecordingState', () => ({
  useRecordingState: () => mockRecordingState(),
}));

// useSettingsStore — only `useBetaChannel` is read.
const mockUseBeta = vi.fn(() => false);
vi.mock('../stores/settings-store', () => ({
  useSettingsStore: <T,>(selector: (s: { useBetaChannel: boolean }) => T) =>
    selector({ useBetaChannel: mockUseBeta() }),
}));

// Import AFTER all mocks so the hook picks them up.
import { useUpdater } from './useUpdater';

describe('useUpdater', () => {
  beforeEach(() => {
    mockInvokeFn.mockReset();
    mockUseBeta.mockReturnValue(false);
    mockRecordingState.mockReturnValue('Idle');
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('starts in idle state', () => {
    const { result } = renderHook(() => useUpdater());
    expect(result.current.status).toBe('idle');
    expect(result.current.updateInfo).toBeNull();
    expect(result.current.shouldNotify).toBe(false);
  });

  it('checkForUpdates surfaces an available update', async () => {
    mockInvokeFn.mockResolvedValueOnce({
      kind: 'available',
      version: '3.1.0',
      body: 'New features',
    });

    const { result } = renderHook(() => useUpdater());

    await act(async () => {
      await result.current.checkForUpdates();
    });

    expect(result.current.status).toBe('available');
    expect(result.current.updateInfo).toEqual({
      version: '3.1.0',
      body: 'New features',
    });
    expect(mockInvokeFn).toHaveBeenCalledWith('check_for_updates_with_channel', { useBeta: false });
  });

  it('checkForUpdates moves to up-to-date when no update is available', async () => {
    mockInvokeFn.mockResolvedValueOnce({ kind: 'no-update' });

    const { result } = renderHook(() => useUpdater());

    await act(async () => {
      await result.current.checkForUpdates();
    });

    expect(result.current.status).toBe('up-to-date');
    expect(result.current.updateInfo).toBeNull();
  });

  it('checkForUpdates resets to idle after 5s on up-to-date so the button reappears', async () => {
    mockInvokeFn.mockResolvedValueOnce({ kind: 'no-update' });

    const { result } = renderHook(() => useUpdater());

    await act(async () => {
      await result.current.checkForUpdates();
    });
    expect(result.current.status).toBe('up-to-date');

    await act(async () => {
      vi.advanceTimersByTime(5_000);
    });
    expect(result.current.status).toBe('idle');
  });

  it('checkForUpdates surfaces an error and resets after 5s', async () => {
    mockInvokeFn.mockRejectedValueOnce(new Error('network down'));

    const { result } = renderHook(() => useUpdater());

    await act(async () => {
      await result.current.checkForUpdates();
    });

    expect(result.current.status).toBe('error');
    expect(result.current.error).toBe('network down');

    await act(async () => {
      vi.advanceTimersByTime(5_000);
    });
    expect(result.current.status).toBe('idle');
  });

  it('shouldNotify is false while a recording is in progress', async () => {
    mockInvokeFn.mockResolvedValueOnce({
      kind: 'available',
      version: '3.1.0',
      body: null,
    });
    mockRecordingState.mockReturnValue('Recording');

    const { result } = renderHook(() => useUpdater());

    await act(async () => {
      await result.current.checkForUpdates();
    });

    expect(result.current.status).toBe('available');
    expect(result.current.shouldNotify).toBe(false);
  });

  it('shouldNotify becomes false after dismiss()', async () => {
    mockInvokeFn.mockResolvedValueOnce({
      kind: 'available',
      version: '3.1.0',
      body: null,
    });

    const { result } = renderHook(() => useUpdater());
    await act(async () => {
      await result.current.checkForUpdates();
    });
    expect(result.current.shouldNotify).toBe(true);

    act(() => {
      result.current.dismiss();
    });
    expect(result.current.shouldNotify).toBe(false);
  });

  it('forwards the beta channel preference to the Rust IPC', async () => {
    mockUseBeta.mockReturnValue(true);
    mockInvokeFn.mockResolvedValueOnce({ kind: 'no-update' });

    const { result } = renderHook(() => useUpdater());
    await act(async () => {
      await result.current.checkForUpdates();
    });

    expect(mockInvokeFn).toHaveBeenCalledWith('check_for_updates_with_channel', { useBeta: true });
  });

  it('restartApp prefers the Rust restart command, falls back to plugin relaunch', async () => {
    mockInvokeFn.mockRejectedValueOnce(new Error('command not registered'));
    const { relaunch } = await import('@tauri-apps/plugin-process');

    const { result } = renderHook(() => useUpdater());
    await act(async () => {
      await result.current.restartApp();
    });

    expect(mockInvokeFn).toHaveBeenCalledWith('restart_app_post_update', undefined);
    expect(relaunch).toHaveBeenCalled();
  });
});
