import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { mockInvoke } from '../test/setup';

type Callback = (e: { payload: unknown }) => void;
const captured: { event: string; cb: Callback }[] = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, cb: Callback) => {
    captured.push({ event, cb });
    return () => {};
  }),
  emit: vi.fn(async () => {}),
}));
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

import { useRecordingControl } from './useRecordingControl';

async function fireState(state: 'Idle' | 'Recording' | 'Processing') {
  const target = captured.find((c) => c.event === 'recording-state-changed');
  if (!target) throw new Error('never subscribed to recording-state-changed');
  await act(async () => { await target.cb({ payload: state }); });
}

const startCalls = () => mockInvoke.mock.calls.filter(([cmd]) => cmd === 'start_recording').length;

describe('useRecordingControl', () => {
  beforeEach(() => {
    captured.length = 0;
    mockInvoke.mockResolvedValue(undefined);
  });

  it('starts capture when Rust enters Recording', async () => {
    renderHook(() => useRecordingControl());
    await act(async () => {});
    await fireState('Recording');
    expect(startCalls()).toBe(1);
  });

  // The trace of 2026-09-14 13:07:32: a finishing dictation turned a new
  // Recording into Idle. Rust closed the microphone, the hook never heard a
  // stop, and the next press was skipped as "already recording".
  it('starts the next recording after Rust ends one without a stop', async () => {
    renderHook(() => useRecordingControl());
    await act(async () => {});
    await fireState('Recording');
    await fireState('Idle');
    await fireState('Recording');
    expect(startCalls()).toBe(2);
  });
});
