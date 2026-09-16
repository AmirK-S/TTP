import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

// Capture registered listeners + their unlisten functions so we can
// simulate Rust event delivery and unmount-during-await races.
type Captured = {
  event: string;
  cb: (e: { payload: unknown }) => void;
  unlistenResolver: (fn: () => void) => void;
  unlistenCalled: boolean;
};
let captured: Captured[] = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    // Hand-roll a Promise whose resolution we control so the test can
    // simulate "unmount before listen() resolves" — the exact race the
    // hook is built to handle.
    let resolver!: (fn: () => void) => void;
    const promise = new Promise<() => void>((res) => {
      resolver = res;
    });
    const entry: Captured = {
      event,
      cb,
      unlistenCalled: false,
      unlistenResolver: (fn) => resolver(() => {
        entry.unlistenCalled = true;
        fn();
      }),
    };
    captured.push(entry);
    return promise;
  }),
}));

import { useTauriEvent } from './useTauriEvent';

function fire(event: string, payload: unknown) {
  const target = captured.find((c) => c.event === event);
  if (!target) throw new Error(`no listener for ${event}`);
  target.cb({ payload });
}

describe('useTauriEvent', () => {
  beforeEach(() => {
    captured = [];
  });

  it('subscribes to the event on mount and routes payloads to the callback', async () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent('foo', handler));

    // Resolve the listen() promise so the unlisten ref is set.
    await act(async () => {
      captured[0].unlistenResolver(() => {});
    });

    // Fire an event — the callback should run.
    await act(async () => {
      fire('foo', { tick: 1 });
    });
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith({ payload: { tick: 1 } });

    unmount();
  });

  it('does NOT invoke the user callback for events delivered after unmount', async () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent('foo', handler));

    await act(async () => {
      captured[0].unlistenResolver(() => {});
    });

    unmount();

    // Late event arrives — must be dropped.
    await act(async () => {
      fire('foo', { late: true });
    });
    expect(handler).not.toHaveBeenCalled();
  });

  it('runs the unlisten function as soon as the listen() promise resolves if already unmounted', async () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent('foo', handler));

    // Unmount BEFORE the listen() promise resolves — this is the exact
    // race the hook is built to handle.
    unmount();

    // Now resolve. The hook must call unlisten immediately, not stash it
    // into a dead ref where it would leak.
    let unlistenInner: (() => void) | null = null;
    await act(async () => {
      captured[0].unlistenResolver(() => {
        unlistenInner = () => {};
      });
    });

    expect(captured[0].unlistenCalled).toBe(true);
    expect(unlistenInner).not.toBeNull();
  });

  it('always reads the most recent callback even when the closure was captured at mount', async () => {
    let lastHandler = vi.fn();
    const { rerender } = renderHook(({ h }) => useTauriEvent('foo', h), {
      initialProps: { h: lastHandler },
    });

    await act(async () => {
      captured[0].unlistenResolver(() => {});
    });

    // Swap the callback to a new fn (very common pattern: parent passes
    // a fresh `useCallback` instance on every render).
    const newer = vi.fn();
    rerender({ h: newer });

    await act(async () => {
      fire('foo', 'payload');
    });

    // The freshly rendered handler must be called — the hook reads via
    // a ref to support this case.
    expect(newer).toHaveBeenCalledWith({ payload: 'payload' });
    expect(lastHandler).not.toHaveBeenCalled();
  });

  it('does not re-subscribe when only the callback identity changes (no churn)', async () => {
    const { rerender } = renderHook(({ h }) => useTauriEvent('foo', h), {
      initialProps: { h: vi.fn() },
    });

    await act(async () => {
      captured[0].unlistenResolver(() => {});
    });
    expect(captured.length).toBe(1);

    rerender({ h: vi.fn() });
    rerender({ h: vi.fn() });
    rerender({ h: vi.fn() });

    // Re-subscribing on every render was the TTP-5 listener-churn root
    // cause. Assert there's still exactly one subscription.
    expect(captured.length).toBe(1);
  });
});
