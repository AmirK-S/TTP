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

/* ==========================================================================
   The completion frame.

   Before Polaris the pill had no completion state. `stage === 'complete'`
   still satisfied `isProcessing`, so a finished dictation kept saying
   "Transcribing…" for another 500 ms and then vanished — identically for a
   paste that was watched arriving, a paste that was only posted, and a paste
   the target demonstrably swallowed. Three of those were in the maintainer's
   log on 2026-08-31 reporting themselves as successes.

   These render the real component and read what a user would see, because the
   whole workstream is about what is on the pill and not about what is in the
   trace.
   ========================================================================== */

import { render, screen, act as rtlAct } from '@testing-library/react';
import { beforeEach, describe as describeSuite, expect as expectFrame, it as itFrame } from 'vitest';
import { listen as listenFn } from '@tauri-apps/api/event';
import type { Event as TauriEvent, EventCallback } from '@tauri-apps/api/event';
import { initI18n, setLanguage } from '../i18n/config';
import { SETTLE_MS } from '../hooks/usePasteCompletion';
import { FloatingBar } from './FloatingBar';

const busHandlers = new Map<string, Set<EventCallback<unknown>>>();

function installEventBus() {
  busHandlers.clear();
  vi.mocked(listenFn).mockImplementation((async (event: string, cb: EventCallback<unknown>) => {
    const set = busHandlers.get(event) ?? new Set();
    set.add(cb);
    busHandlers.set(event, set);
    return () => set.delete(cb);
  }) as unknown as typeof listenFn);
}

function emitBus(event: string, payload: unknown) {
  rtlAct(() => {
    for (const cb of busHandlers.get(event) ?? []) cb({ payload } as TauriEvent<unknown>);
  });
}

/** Complete a dictation with the given outcome and let the 140 ms hold expire. */
function completeWith(outcome: string | null) {
  emitBus('transcription-progress', {
    stage: 'complete',
    message: '',
    ...(outcome ? { params: { outcome } } : {}),
  });
  rtlAct(() => { vi.advanceTimersByTime(SETTLE_MS); });
}

/** Render, and flush the mount effects that await `listen()` / `safeInvoke`. */
async function mountBar() {
  const view = render(<FloatingBar />);
  await rtlAct(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
  return view;
}

describeSuite('the pill at the end of a dictation', () => {
  beforeEach(() => {
    initI18n('en');
    setLanguage('en');
    installEventBus();
    // The first-launch check reads `localStorage` on the failure path, and
    // `safeInvoke` fails here because nothing answers `is_first_launch_cmd`.
    // Not what these tests are about; give it somewhere to write.
    vi.stubGlobal('localStorage', {
      getItem: () => 'true',
      setItem: () => {},
      removeItem: () => {},
    });
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.mocked(listenFn).mockReset();
    vi.unstubAllGlobals();
  });

  itFrame('says nothing new when this build of Rust reports no outcome', async () => {
    // `emit_progress(app, "complete", "", None)` — today's binary. The change
    // is a strict no-op against it: no frame, no mark, and the pre-existing
    // behaviour (still "Transcribing…" for the residual 500 ms) intact. The
    // frontend must never fill the gap by guessing.
    const { container } = await mountBar();
    completeWith(null);
    expectFrame(screen.getByText('Transcribing…')).toBeInTheDocument();
    expectFrame(container.querySelector('[data-ttp-mark]')).toBeNull();
  });

  itFrame('marks an observed paste, and does not congratulate itself about it', async () => {
    const { container } = await mountBar();
    completeWith('pasted');
    // The double tick — "delivered", not merely "sent".
    expectFrame(container.querySelector('[data-ttp-mark="arrived"]')).not.toBeNull();
    // No words. The user is looking at their own text; saying "Pasted" on
    // every dictation is the self-report this wave took out of the trace.
    expectFrame(screen.queryByText(/Sent/)).toBeNull();
    expectFrame(screen.queryByText('Transcribing…')).toBeNull();
    // The live region still gets a whole sentence: a screen-reader user
    // cannot look at the text field.
    expectFrame(screen.getByText('The text arrived.')).toBeInTheDocument();
  });

  itFrame('reports an unverified paste in words, quietly', async () => {
    const { container } = await mountBar();
    completeWith('pasted_unverified');
    expectFrame(container.querySelector('[data-ttp-mark="sent"]')).not.toBeNull();
    expectFrame(screen.getByText('Sent — not confirmed')).toBeInTheDocument();
    expectFrame(
      screen.getByText('The text was sent. Nothing confirmed it arrived.'),
    ).toBeInTheDocument();
  });

  /* The invariant the whole design hangs on. This state will be a large
     minority of dictations — 40% of the corpus could not be seen at all —
     and on every one of them nothing has gone wrong. If it ever shakes, or
     turns red, it has become an alarm that fires several times a day, and
     the user will learn to ignore the one that matters. */
  itFrame('never alarms for an unverified paste', async () => {
    const { container } = await mountBar();
    completeWith('pasted_unverified');
    expectFrame(container.querySelector('.anim-shake')).toBeNull();
    expectFrame(container.querySelector('.bg-app-danger\\/95')).toBeNull();
    // And nothing to dismiss: the window does not take pointer events at all.
    expectFrame(container.firstElementChild?.className).toContain('pointer-events-none');
  });

  itFrame('trembles for a swallowed paste and says where the text is', async () => {
    const { container } = await mountBar();
    completeWith('paste_swallowed');
    expectFrame(screen.getByText("Nothing arrived — it's in Settings → History")).toBeInTheDocument();
    expectFrame(container.querySelector('.anim-shake')).not.toBeNull();
  });

  itFrame('speaks French', async () => {
    setLanguage('fr');
    await mountBar();
    completeWith('pasted_unverified');
    expectFrame(screen.getByText('Envoyé — non confirmé')).toBeInTheDocument();
    // `beforeEach` puts it back to English; doing it here would re-render a
    // still-mounted component outside `act`.
  });

  /* `recording-state-changed` arrives on the key press, ahead of any progress
     event, so a fast second dictation would otherwise draw its waveform and
     the previous dictation's report in the same pill. */
  itFrame('yields to the next recording immediately', async () => {
    const { container } = await mountBar();
    completeWith('pasted_unverified');
    emitBus('recording-state-changed', 'Recording');
    expectFrame(container.querySelector('[data-ttp-mark]')).toBeNull();
    expectFrame(screen.queryByText('Sent — not confirmed')).toBeNull();
  });

  itFrame('drops the frame when the next dictation starts', async () => {
    const { container } = await mountBar();
    completeWith('pasted_unverified');
    emitBus('transcription-progress', { stage: 'transcribing', message: 'progress.transcribing' });
    expectFrame(container.querySelector('[data-ttp-mark]')).toBeNull();
    expectFrame(screen.getByText('Transcribing…')).toBeInTheDocument();
  });
});
