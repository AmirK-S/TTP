// TTP - Talk To Paste
// Tests for the floating bar's reduced-motion subscription.
//
// The shipped code read `matchMedia(...).matches` once at render with no
// `change` listener, so a user who turned Reduce Motion on while TTP was
// running kept the old behaviour until the window reloaded — and the floating
// bar window essentially never reloads. Adding the media query is not the
// same as honouring it; this checks it actually reacts.

import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { usePrefersReducedMotion } from './FloatingBar';

/** A MediaQueryList stub whose value we can change from the test. */
function installMatchMedia(initial: boolean) {
  const listeners = new Set<(e: MediaQueryListEvent) => void>();
  let matches = initial;
  const queries: string[] = [];

  const query = {
    get matches() {
      return matches;
    },
    media: '(prefers-reduced-motion: reduce)',
    addEventListener: (type: string, fn: (e: MediaQueryListEvent) => void) => {
      if (type === 'change') listeners.add(fn);
    },
    removeEventListener: (type: string, fn: (e: MediaQueryListEvent) => void) => {
      if (type === 'change') listeners.delete(fn);
    },
  };

  vi.stubGlobal('matchMedia', (q: string) => {
    queries.push(q);
    return query as unknown as MediaQueryList;
  });

  return {
    queries,
    listenerCount: () => listeners.size,
    set(next: boolean) {
      matches = next;
      for (const fn of listeners) fn({ matches: next } as MediaQueryListEvent);
    },
  };
}

describe('usePrefersReducedMotion', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('asks the right query', () => {
    const mm = installMatchMedia(false);
    renderHook(() => usePrefersReducedMotion());
    expect(mm.queries).toContain('(prefers-reduced-motion: reduce)');
  });

  it('reports the preference that is already set at mount', () => {
    installMatchMedia(true);
    const { result } = renderHook(() => usePrefersReducedMotion());
    expect(result.current).toBe(true);
  });

  it('reacts when the user turns Reduce Motion ON mid-session', () => {
    const mm = installMatchMedia(false);
    const { result } = renderHook(() => usePrefersReducedMotion());
    expect(result.current).toBe(false);

    act(() => mm.set(true));
    expect(result.current).toBe(true);
  });

  it('reacts when the user turns it back OFF', () => {
    const mm = installMatchMedia(true);
    const { result } = renderHook(() => usePrefersReducedMotion());
    act(() => mm.set(false));
    expect(result.current).toBe(false);
  });

  it('unsubscribes on unmount', () => {
    const mm = installMatchMedia(false);
    const { unmount } = renderHook(() => usePrefersReducedMotion());
    expect(mm.listenerCount()).toBe(1);
    unmount();
    expect(mm.listenerCount()).toBe(0);
  });

  it('survives an environment with no matchMedia at all', () => {
    vi.stubGlobal('matchMedia', undefined);
    const { result } = renderHook(() => usePrefersReducedMotion());
    expect(result.current).toBe(false);
  });
});
