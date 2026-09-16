import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

// Capture the callback registered with `listen` so the test can fire events
// at will. Each call to `listen` is recorded; tests can dispatch via the
// captured callback to simulate the Rust event loop.
type Callback = (e: { payload: unknown }) => void;
const captured: { event: string; cb: Callback }[] = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, cb: Callback) => {
    captured.push({ event, cb });
    // Unlisten — no-op for our purposes.
    return () => {
      const idx = captured.findIndex((c) => c.cb === cb);
      if (idx !== -1) captured.splice(idx, 1);
    };
  }),
  emit: vi.fn(),
}));

// Import after mocks so the hook picks them up.
import { useTranscription } from './useTranscription';

function fireProgress(payload: { stage: string; message?: string; params?: Record<string, string | number> }) {
  const target = captured.find((c) => c.event === 'transcription-progress');
  if (!target) throw new Error('useTranscription never subscribed to transcription-progress');
  target.cb({ payload });
}

describe('useTranscription', () => {
  beforeEach(() => {
    captured.length = 0;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('starts in idle state with empty message', async () => {
    const { result } = renderHook(() => useTranscription());

    // Let the useTauriEvent effect register the listener.
    await act(async () => {});

    expect(result.current.stage).toBe('idle');
    expect(result.current.message).toBe('');
    expect(result.current.isProcessing).toBe(false);
  });

  it('updates state when the pipeline transitions into transcribing', async () => {
    const { result } = renderHook(() => useTranscription());
    await act(async () => {});

    await act(async () => {
      fireProgress({ stage: 'transcribing', message: 'progress.transcribing' });
    });

    expect(result.current.stage).toBe('transcribing');
    expect(result.current.message).toBe('progress.transcribing');
    expect(result.current.isProcessing).toBe(true);
  });

  it('resets to idle 500ms after a complete event', async () => {
    const { result } = renderHook(() => useTranscription());
    await act(async () => {});

    await act(async () => {
      fireProgress({ stage: 'complete', message: '' });
    });
    expect(result.current.stage).toBe('complete');

    await act(async () => {
      vi.advanceTimersByTime(499);
    });
    expect(result.current.stage).toBe('complete'); // not yet

    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current.stage).toBe('idle');
    expect(result.current.message).toBe('');
  });

  it('resets to idle 4000ms after an error event (long enough for user to read)', async () => {
    const { result } = renderHook(() => useTranscription());
    await act(async () => {});

    await act(async () => {
      fireProgress({ stage: 'error', message: 'error.no_speech' });
    });
    expect(result.current.stage).toBe('error');
    expect(result.current.message).toBe('error.no_speech');

    await act(async () => {
      vi.advanceTimersByTime(3999);
    });
    expect(result.current.stage).toBe('error');

    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current.stage).toBe('idle');
  });

  it('cancels a pending reset when a new event arrives before the timer fires', async () => {
    const { result } = renderHook(() => useTranscription());
    await act(async () => {});

    await act(async () => {
      fireProgress({ stage: 'error', message: 'error.no_speech' });
    });

    // Half-way through the 4s reset, a new transcription kicks off.
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    await act(async () => {
      fireProgress({ stage: 'transcribing', message: 'progress.transcribing' });
    });

    // Run out the original 4s timer — should NOT reset us back to idle.
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(result.current.stage).toBe('transcribing');
  });

  it('passes through interpolation params for the consumer', async () => {
    const { result } = renderHook(() => useTranscription());
    await act(async () => {});

    await act(async () => {
      fireProgress({
        stage: 'error',
        message: 'error.api_generic',
        params: { status: 503, body: 'oops' },
      });
    });

    expect(result.current.params).toEqual({ status: 503, body: 'oops' });
  });
});
