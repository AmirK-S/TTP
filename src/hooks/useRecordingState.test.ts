import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

// Capture the registered callback so the test can dispatch events.
type Callback = (e: { payload: unknown }) => void;
const captured: { event: string; cb: Callback }[] = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, cb: Callback) => {
    captured.push({ event, cb });
    return () => {
      const idx = captured.findIndex((c) => c.cb === cb);
      if (idx !== -1) captured.splice(idx, 1);
    };
  }),
}));

import { useRecordingState } from './useRecordingState';

function fire(payload: 'Idle' | 'Recording' | 'Processing') {
  const target = captured.find((c) => c.event === 'recording-state-changed');
  if (!target) throw new Error('useRecordingState never subscribed');
  target.cb({ payload });
}

describe('useRecordingState', () => {
  beforeEach(() => {
    captured.length = 0;
  });

  it('starts in Idle', async () => {
    const { result } = renderHook(() => useRecordingState());
    await act(async () => {});
    expect(result.current).toBe('Idle');
  });

  it('updates as Rust emits state changes', async () => {
    const { result } = renderHook(() => useRecordingState());
    await act(async () => {});

    await act(async () => {
      fire('Recording');
    });
    expect(result.current).toBe('Recording');

    await act(async () => {
      fire('Processing');
    });
    expect(result.current).toBe('Processing');

    await act(async () => {
      fire('Idle');
    });
    expect(result.current).toBe('Idle');
  });

  it('subscribes exactly once per mount (no churn)', async () => {
    renderHook(() => useRecordingState());
    renderHook(() => useRecordingState());
    await act(async () => {});
    // Two hook instances → two subscriptions. The audit found TTP-5
    // was caused by the SAME hook re-subscribing on every render of one
    // component; we assert here that one mount yields one subscription.
    expect(captured.filter((c) => c.event === 'recording-state-changed').length).toBe(2);
  });
});
