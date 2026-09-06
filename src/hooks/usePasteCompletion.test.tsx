// TTP - Talk To Paste
// When the pill is allowed to commit to a reading, and what may change it
// afterwards.
//
// The timing here is not arbitrary and is the reason this hook exists at all.
// Per `docs/tracing.md`, `paste.verify` settles a median of 44 ms after
// `dictation.finish` and later than it in 78% of dictations, so the
// `verification` on the completion event is `pending` — and therefore
// `pasted_unverified` — four times out of five, *including on the dictations
// we did observe*. Drawing that verbatim would put "sent, not confirmed" on
// almost everything, which is a fresh lie pointing the other way.
//
// So: hold 140 ms, take the settled verdict if it arrives, and past that
// accept only the one late verdict a user can act on.

import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import type { Event, EventCallback } from '@tauri-apps/api/event';
import { PASTE_VERIFIED_EVENT, SETTLE_MS, usePasteCompletion } from './usePasteCompletion';

/** Handlers registered through the mocked `listen`, by event name. */
const handlers = new Map<string, Set<EventCallback<unknown>>>();

beforeEach(() => {
  handlers.clear();
  vi.useFakeTimers();
  vi.mocked(listen).mockImplementation((async (event: string, cb: EventCallback<unknown>) => {
    const set = handlers.get(event) ?? new Set();
    set.add(cb);
    handlers.set(event, set);
    return () => set.delete(cb);
  }) as unknown as typeof listen);
});

afterEach(() => {
  vi.useRealTimers();
  vi.mocked(listen).mockReset();
});

/** Flush the `listen()` promise `useTauriEvent` awaits before subscribing. */
async function settleSubscription() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function verify(outcome: string) {
  act(() => {
    for (const cb of handlers.get(PASTE_VERIFIED_EVENT) ?? []) {
      cb({ payload: { outcome } } as Event<unknown>);
    }
  });
}

/** Render the hook with a rerenderable `(stage, params)` pair. */
function mount() {
  return renderHook(
    ({ stage, params }: { stage: string; params?: Record<string, string | number> }) =>
      usePasteCompletion(stage, params),
    { initialProps: { stage: 'idle' } as { stage: string; params?: Record<string, string | number> } },
  );
}

describe('usePasteCompletion', () => {
  it('draws nothing at all when Rust did not name an outcome', async () => {
    // Today's binary: `emit_progress(app, "complete", "", None)`. The pill has
    // no basis for any of the four frames, so it draws none of them and
    // behaves exactly as it did before this workstream. It must not guess.
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete' });
    act(() => { vi.advanceTimersByTime(5000); });
    expect(result.current).toBeNull();
  });

  it('holds for Law 3’s 140 ms before committing to anything', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    // Nothing drawn yet: the median verdict lands at 44 ms and we are not
    // going to paint a reading we are about to replace.
    act(() => { vi.advanceTimersByTime(SETTLE_MS - 1); });
    expect(result.current).toBeNull();
    act(() => { vi.advanceTimersByTime(1); });
    expect(result.current).toBe('pasted_unverified');
  });

  /* The 78% case, and the whole reason for the second event. */
  it('upgrades a pending completion when the verdict lands inside the hold', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    act(() => { vi.advanceTimersByTime(44); }); // the corpus median
    verify('pasted');
    act(() => { vi.advanceTimersByTime(SETTLE_MS); });
    expect(result.current).toBe('pasted');
  });

  it('downgrades inside the hold too — evidence wins in both directions', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted' } });
    verify('paste_swallowed');
    act(() => { vi.advanceTimersByTime(SETTLE_MS); });
    expect(result.current).toBe('paste_swallowed');
  });

  /* Past the hold the mark stops moving. It was honest at the instant it was
     drawn — the rule `dictation.finish` follows — and flipping one tick into
     two half a second later is a flicker that tells the user nothing they can
     do anything about. */
  it('does not upgrade a mark it has already drawn', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    act(() => { vi.advanceTimersByTime(SETTLE_MS); });
    expect(result.current).toBe('pasted_unverified');
    verify('pasted');
    expect(result.current).toBe('pasted_unverified');
  });

  /* The asymmetry, and the one place a late verdict is allowed to interrupt.
     A swallow means the user's words are not on screen and are recoverable
     from Settings → History while they are being told. */
  it('re-opens for a swallow that arrives after the frame committed', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    act(() => { vi.advanceTimersByTime(SETTLE_MS); });
    verify('paste_swallowed');
    expect(result.current).toBe('paste_swallowed');
  });

  it('re-opens for a swallow that arrives after the frame closed', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    act(() => { vi.advanceTimersByTime(SETTLE_MS + 1500); });
    expect(result.current).toBeNull();
    // p90 for the verdict is 616 ms; the pill has long since gone quiet.
    verify('paste_swallowed');
    expect(result.current).toBe('paste_swallowed');
  });

  it('does not re-open twice for the same swallow', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'paste_swallowed' } });
    act(() => { vi.advanceTimersByTime(SETTLE_MS + 3000); });
    verify('paste_swallowed');
    // Still the same frame, running out its original 4 s rather than
    // restarting it.
    act(() => { vi.advanceTimersByTime(1000); });
    expect(result.current).toBeNull();
  });

  describe('closing itself', () => {
    it('clears the observed frame after its hold', async () => {
      const { result, rerender } = mount();
      await settleSubscription();
      rerender({ stage: 'complete', params: { outcome: 'pasted' } });
      act(() => { vi.advanceTimersByTime(SETTLE_MS + 799); });
      expect(result.current).toBe('pasted');
      act(() => { vi.advanceTimersByTime(1); });
      expect(result.current).toBeNull();
    });

    /* `useTranscription` returns the stage to `idle` 500 ms after `complete`.
       The unverified frame runs to 1500 ms because it has words in it that
       have to be readable, so it must not be cut short by that reset. */
    it('outlives the stage going idle underneath it', async () => {
      const { result, rerender } = mount();
      await settleSubscription();
      rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
      act(() => { vi.advanceTimersByTime(SETTLE_MS); });
      rerender({ stage: 'idle' });
      act(() => { vi.advanceTimersByTime(600); });
      expect(result.current).toBe('pasted_unverified');
    });

    it('gets out of the way the moment the next dictation starts', async () => {
      const { result, rerender } = mount();
      await settleSubscription();
      rerender({ stage: 'complete', params: { outcome: 'paste_swallowed' } });
      act(() => { vi.advanceTimersByTime(SETTLE_MS); });
      expect(result.current).toBe('paste_swallowed');
      rerender({ stage: 'transcribing' });
      expect(result.current).toBeNull();
    });

    it('yields the pill to an error frame', async () => {
      const { result, rerender } = mount();
      await settleSubscription();
      rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
      act(() => { vi.advanceTimersByTime(SETTLE_MS); });
      rerender({ stage: 'error' });
      expect(result.current).toBeNull();
    });
  });

  it('ignores a verdict it cannot read', async () => {
    const { result, rerender } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    verify('something_else');
    act(() => { vi.advanceTimersByTime(SETTLE_MS); });
    expect(result.current).toBe('pasted_unverified');
  });

  it('leaves no timer running after unmount', async () => {
    const { rerender, unmount } = mount();
    await settleSubscription();
    rerender({ stage: 'complete', params: { outcome: 'pasted_unverified' } });
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
