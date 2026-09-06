// TTP - Talk To Paste
// The pill's completion frame: which of the four outcomes to draw, and when to
// stop waiting for a better answer.
//
// ── Why this is not just a field on the progress event ──────────────────────
//
// Verification runs off the dictation's critical path and nothing waits for
// it. `docs/tracing.md`: `paste.verify` lands a median of 44 ms after
// `dictation.finish`, a 90th percentile of 616 ms, and *later than it in 78%
// of dictations*. So at the instant Rust emits `complete`, the honest
// `verification` is `pending` four times out of five, and `pending` maps onto
// `pasted_unverified`.
//
// Rendering that verbatim would put "sent, not confirmed" on almost every
// dictation, including the ones we did observe — which is a new lie in the
// opposite direction, and a far more annoying one. Two things fix it:
//
//   1. **A settled event.** Rust must emit the verdict again when the verifier
//      concludes, not only at `complete`. See `PASTE_VERIFIED_EVENT`.
//   2. **A 140 ms hold before committing.** Which is not a number invented
//      here: `REACTION_DELAY_MS` is the Companion's Law 3 delay, already
//      applied to every reaction caused by the app's own result rather than by
//      the user's key. The median verdict (44 ms) lands comfortably inside it,
//      so the majority of dictations resolve before anything is drawn, and the
//      delay costs nothing because the frame was going to be late anyway.
//
// ── Upgrades are ignored, one downgrade is not ──────────────────────────────
//
// Past the hold, a verdict that arrives late does **not** flip a single tick
// into a double tick. The mark was honest at the instant it was drawn — the
// same rule `dictation.finish` follows — and a pill that silently upgrades
// itself half a second later is a flicker that tells the user nothing they can
// act on.
//
// `paste_swallowed` is the exception, in both directions: it may arrive after
// the frame has committed, and it may arrive after the frame has closed
// entirely, and in both cases it re-opens the pill. The asymmetry is the whole
// point. An upgrade from "did not see it" to "saw it" changes nothing for the
// user. A downgrade to "it did not arrive" changes everything, and the text is
// still recoverable while they are told.
//
// Nothing here demands anything: every frame closes itself on a timer, the
// window is `pointer-events-none`, and there is no state that survives the
// dictation that produced it.

import { useEffect, useRef, useState } from 'react';
import { useTauriEvent } from './useTauriEvent';
import { REACTION_DELAY_MS } from '../components/ui/companion-timing';
import { asPasteOutcome, treatmentFor, type PasteOutcome } from '../lib/pasteOutcome';

/**
 * The event Rust must emit when `spawn_paste_verification` records its
 * verdict. Payload: `{ outcome, verification }`, the same two slugs
 * `dictation.finish` already writes.
 *
 * It does not exist yet. Until it does, this hook still works — it just never
 * sees an upgrade, so a `pending` completion commits as `pasted_unverified`
 * after the hold, which is exactly what the trace line says at that instant.
 */
export const PASTE_VERIFIED_EVENT = 'paste-verified';

/** How long to wait for a settled verdict before drawing. Law 3's 140 ms. */
export const SETTLE_MS = REACTION_DELAY_MS;

interface PasteVerifiedPayload {
  outcome?: unknown;
  verification?: unknown;
}

/**
 * @param stage    the transcription stage from `useTranscription`.
 * @param params   that event's `params`, where Rust must put `outcome`.
 * @returns the outcome to draw, or `null` for no completion frame at all.
 */
export function usePasteCompletion(
  stage: string,
  params?: Record<string, string | number> | undefined,
): PasteOutcome | null {
  const [shown, setShown] = useState<PasteOutcome | null>(null);

  // Timers, and the outcome the hold is currently sitting on. Refs rather than
  // state: a verdict landing inside the hold must replace the candidate
  // without causing a render, or the pill would draw the value we are still
  // deciding not to trust.
  const settleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const holdTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const candidate = useRef<PasteOutcome | null>(null);
  const settling = useRef(false);
  const shownRef = useRef<PasteOutcome | null>(null);

  const clearTimers = () => {
    if (settleTimer.current) clearTimeout(settleTimer.current);
    if (holdTimer.current) clearTimeout(holdTimer.current);
    settleTimer.current = null;
    holdTimer.current = null;
    settling.current = false;
  };

  const commit = (outcome: PasteOutcome) => {
    settling.current = false;
    settleTimer.current = null;
    if (holdTimer.current) clearTimeout(holdTimer.current);
    shownRef.current = outcome;
    setShown(outcome);
    holdTimer.current = setTimeout(() => {
      holdTimer.current = null;
      shownRef.current = null;
      setShown(null);
    }, treatmentFor(outcome).holdMs);
  };

  useEffect(() => clearTimers, []);

  /* A dictation reaching `complete` opens the hold. Anything else that is not
     `idle` — a new recording being transcribed, or an error frame taking the
     pill over — closes whatever was on screen.

     `idle` deliberately does nothing. `useTranscription` drops back to it
     500 ms after `complete`, and the unverified frame outlives that by a
     second; the frame's lifetime is this hook's, not the stage's. */
  useEffect(() => {
    if (stage === 'complete') {
      const outcome = asPasteOutcome(params?.outcome);
      // No slug on the wire: this build of Rust cannot tell the four apart, so
      // neither can the pill, so it draws no completion frame and behaves
      // exactly as it did before this workstream. Guessing here is how the bug
      // gets reimplemented on the frontend.
      if (!outcome) return;
      clearTimers();
      candidate.current = outcome;
      settling.current = true;
      settleTimer.current = setTimeout(() => {
        settleTimer.current = null;
        if (candidate.current) commit(candidate.current);
      }, SETTLE_MS);
      return;
    }
    if (stage === 'idle') return;
    clearTimers();
    candidate.current = null;
    shownRef.current = null;
    setShown(null);
    // `params` is a fresh object per event, so a second dictation completing
    // with the same stage still re-runs this.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stage, params]);

  useTauriEvent<PasteVerifiedPayload>(PASTE_VERIFIED_EVENT, (event) => {
    const outcome = asPasteOutcome(event.payload?.outcome);
    if (!outcome) return;

    // Inside the hold: the settled verdict always beats the one `complete`
    // carried, in either direction. Nothing has been drawn yet, so there is
    // nothing to flicker.
    if (settling.current) {
      candidate.current = outcome;
      return;
    }

    // Past the hold, only the bad news gets through — and it gets through even
    // if the frame had already closed.
    if (outcome === 'paste_swallowed' && shownRef.current !== 'paste_swallowed') {
      commit(outcome);
    }
  });

  return shown;
}
