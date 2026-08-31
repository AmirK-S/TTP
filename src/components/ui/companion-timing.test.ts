// TTP - Talk To Paste
// Tests for the Companion face's clock.
//
// The design doc's claim is that the character lives in *when* things happen.
// If that is true then these numbers are the feature, and they are the thing
// worth pinning down. Everything here runs against a virtual clock with an
// injected random source — no DOM, no timers, no flake.

import { describe, expect, it, vi } from 'vitest';
import {
  BLINK_CLOSE_MS,
  BLINK_HOLD_MS,
  BLINK_MIN_MS,
  BLINK_OPEN_MS,
  BLINK_SPREAD_MS,
  BLINK_TOTAL_MS,
  BREATH_AMPLITUDE,
  BREATH_PERIOD_MS,
  DOUBLE_BLINK_CHANCE,
  ERROR_SHAKE_MS,
  EYE_HEIGHT,
  EYE_OPACITY,
  EYE_SHUT_HEIGHT,
  REACTION_DELAY_MS,
  RESOLVE_PEAK_HEIGHT,
  RESOLVE_SETTLE,
  RESOLVE_TOTAL_MS,
  breathPhaseOffsetMs,
  createBlinkScheduler,
  cssTransition,
  doubleBlinkGap,
  isDoubleBlink,
  nextBlinkDelay,
  reactionDelayFor,
  transitionFor,
  type BlinkPhase,
} from './companion-timing';

/* -- A virtual clock, so nothing here waits on wall time ------------------- */

function makeClock() {
  let now = 0;
  let nextId = 1;
  const pending = new Map<number, { at: number; fn: () => void }>();

  return {
    now: () => now,
    setTimer(fn: () => void, ms: number): number {
      const id = nextId++;
      pending.set(id, { at: now + ms, fn });
      return id;
    },
    clearTimer(id: number) {
      pending.delete(id);
    },
    /** Run every timer due at or before `now + ms`, in due order. */
    advance(ms: number) {
      const target = now + ms;
      for (;;) {
        let dueId: number | null = null;
        let dueAt = Infinity;
        for (const [id, entry] of pending) {
          if (entry.at <= target && entry.at < dueAt) {
            dueAt = entry.at;
            dueId = id;
          }
        }
        if (dueId === null) break;
        const entry = pending.get(dueId)!;
        pending.delete(dueId);
        now = entry.at;
        entry.fn();
      }
      now = target;
    },
    pendingCount: () => pending.size,
  };
}

/** A random source that yields a scripted sequence, then repeats the last value. */
function scriptedRandom(values: number[]) {
  let i = 0;
  return () => values[Math.min(i++, values.length - 1)];
}

describe('blink interval — bounded irregularity (Law 1)', () => {
  it('spans 4.2 s to 7.8 s', () => {
    expect(nextBlinkDelay(0)).toBe(4200);
    expect(nextBlinkDelay(1)).toBe(7800);
    expect(nextBlinkDelay(0.5)).toBe(6000);
  });

  it('never returns the shipped 5200 ms constant twice in a row from distinct draws', () => {
    // The regression this whole workstream exists for: a fixed 5200 ms period
    // is a regularity a person finds, and finding it kills the illusion.
    const draws = Array.from({ length: 200 }, (_, i) => nextBlinkDelay(i / 200));
    expect(new Set(draws).size).toBe(200);
  });

  it('is bounded — never unbounded-random, which reads as broken', () => {
    for (let r = 0; r < 1; r += 0.01) {
      const d = nextBlinkDelay(r);
      expect(d).toBeGreaterThanOrEqual(BLINK_MIN_MS);
      expect(d).toBeLessThanOrEqual(BLINK_MIN_MS + BLINK_SPREAD_MS);
    }
  });

  it('averages roughly 6 s — about ten blinks a minute', () => {
    const mean = nextBlinkDelay(0.5);
    expect(mean).toBe(6000);
    expect(60_000 / mean).toBeCloseTo(10, 5);
  });
});

describe('blink shape — the asymmetry is load-bearing', () => {
  it('closes faster than it opens', () => {
    // A symmetric transition reads as a slow deliberate wink, which is a
    // communicative gesture and therefore a demand. This face demands nothing.
    expect(BLINK_CLOSE_MS).toBe(90);
    expect(BLINK_OPEN_MS).toBe(130);
    expect(BLINK_CLOSE_MS).toBeLessThan(BLINK_OPEN_MS);
  });

  it('holds shut for 40 ms and totals 260 ms', () => {
    expect(BLINK_HOLD_MS).toBe(40);
    expect(BLINK_TOTAL_MS).toBe(260);
  });

  it('closes to a 1px line rather than nothing — a collapsed rect vanishes', () => {
    expect(EYE_SHUT_HEIGHT).toBe(1);
    expect(EYE_SHUT_HEIGHT).toBeGreaterThan(0);
  });
});

describe('double blinks', () => {
  it('fire on about 12% of blinks', () => {
    expect(DOUBLE_BLINK_CHANCE).toBe(0.12);
    expect(isDoubleBlink(0.11)).toBe(true);
    expect(isDoubleBlink(0.12)).toBe(false);
    expect(isDoubleBlink(0.9)).toBe(false);
  });

  it('starts the second blink 180-260 ms after the first finishes', () => {
    expect(doubleBlinkGap(0)).toBe(180);
    expect(doubleBlinkGap(1)).toBe(260);
  });
});

describe('Law 3 — who caused it decides when it happens', () => {
  it('reacts to the user immediately', () => {
    expect(reactionDelayFor('user')).toBe(0);
    expect(transitionFor('listening').delay).toBe(0);
  });

  it("delays reactions to the app's own result into the 120-200 ms band", () => {
    expect(reactionDelayFor('app')).toBe(140);
    expect(reactionDelayFor('app')).toBeGreaterThanOrEqual(120);
    expect(reactionDelayFor('app')).toBeLessThanOrEqual(200);
    expect(transitionFor('resolving').delay).toBe(REACTION_DELAY_MS);
  });
});

describe('the states', () => {
  it('never shuts the eyes while thinking — it narrows and holds', () => {
    // The shipped face went blank for the whole of transcription, at exactly
    // the moment the user is staring hardest. Any state that can last more
    // than ~4 s needs a stable appearance, not an animation you can count.
    expect(EYE_HEIGHT.thinking).toBe(2.2);
    expect(EYE_HEIGHT.thinking).toBeGreaterThan(EYE_SHUT_HEIGHT);
  });

  it('is no louder while listening than at idle', () => {
    expect(EYE_HEIGHT.listening).toBeLessThanOrEqual(EYE_HEIGHT.idle);
    expect(EYE_OPACITY.listening).toBeLessThan(EYE_OPACITY.idle);
  });

  it('is quieter at idle than the shipped face was', () => {
    expect(EYE_HEIGHT.idle).toBe(3.0); // was 3.5
    expect(EYE_OPACITY.idle).toBe(0.85); // was 0.90
  });

  it('holds completely still through the error shake, then settles', () => {
    const t = transitionFor('error');
    expect(t.delay).toBeGreaterThanOrEqual(ERROR_SHAKE_MS);
    expect(t.delay).toBe(520);
    expect(t.duration).toBe(160);
    expect(EYE_HEIGHT.error).toBe(2.2);
  });
});

describe('the resolve beat — the moment the words land', () => {
  it('is late, short, and over in 400 ms', () => {
    expect(REACTION_DELAY_MS).toBe(140);
    expect(transitionFor('resolving').duration).toBe(90);
    expect(RESOLVE_SETTLE.duration).toBe(170);
    // 260 ms of beat, 140 ms of delay before it.
    expect(RESOLVE_TOTAL_MS - REACTION_DELAY_MS).toBe(260);
    expect(RESOLVE_TOTAL_MS).toBe(400);
  });

  it('fits inside the 500 ms that `stage === "complete"` is held for', () => {
    expect(RESOLVE_TOTAL_MS).toBeLessThan(500);
  });

  it('overshoots by exactly 0.6px, and only inward', () => {
    expect(RESOLVE_PEAK_HEIGHT - EYE_HEIGHT.resolving).toBeCloseTo(0.6, 10);
    // It never moves toward the viewer: the overshoot is in eye height inside
    // a fixed 16x16 box, not scale, not position, not growth.
    expect(RESOLVE_PEAK_HEIGHT).toBeLessThan(16);
  });

  it('returns fully to rest — no residue', () => {
    expect(EYE_HEIGHT.resolving).toBe(EYE_HEIGHT.idle);
  });
});

describe('the breath', () => {
  it('stays under the peripheral texture threshold of ~1px per ~2s', () => {
    // ±6% of a 3.0px eye is ±0.18px of eye height.
    const amplitudePx = EYE_HEIGHT.idle * BREATH_AMPLITUDE;
    expect(amplitudePx).toBeCloseTo(0.18, 10);

    // Over one period the height goes down, up, and back: 4 x amplitude of
    // total travel. Law 2 says a change of less than ~1px over more than ~2s
    // is texture rather than an event, and events are what steal a saccade.
    const travelPerPeriodPx = 4 * amplitudePx;
    const travelPerTwoSecondsPx = travelPerPeriodPx * (2000 / BREATH_PERIOD_MS);
    expect(travelPerTwoSecondsPx).toBeCloseTo(0.313, 3);
    expect(travelPerTwoSecondsPx).toBeLessThan(1);
  });

  it('randomises its phase across a full period, as a negative delay', () => {
    expect(breathPhaseOffsetMs(0)).toBe(-0);
    expect(breathPhaseOffsetMs(1)).toBe(-BREATH_PERIOD_MS);
    expect(breathPhaseOffsetMs(0.5)).toBe(-2300);
  });
});

describe('cssTransition', () => {
  it('renders delay and easing into the shorthand', () => {
    expect(cssTransition('all', transitionFor('resolving'))).toBe('all 90ms ease-out 140ms');
    expect(cssTransition('all', transitionFor('thinking'))).toBe('all 180ms ease-in-out 0ms');
  });
});

/* -- The scheduler --------------------------------------------------------- */

describe('createBlinkScheduler', () => {
  const setup = (randoms: number[]) => {
    const clock = makeClock();
    const phases: Array<{ at: number; phase: BlinkPhase }> = [];
    const scheduler = createBlinkScheduler({
      random: scriptedRandom(randoms),
      setTimer: clock.setTimer,
      clearTimer: clock.clearTimer,
      onPhase: (phase) => phases.push({ at: clock.now(), phase }),
    });
    return { clock, phases, scheduler };
  };

  it('runs the close/hold/open chain with the specified asymmetry', () => {
    // 0.5 → first interval 6000 ms; 0.9 → not a double blink.
    const { clock, phases, scheduler } = setup([0.5, 0.9]);
    scheduler.start();
    clock.advance(10_000);

    expect(phases.slice(0, 3)).toEqual([
      { at: 6000, phase: 'closing' },
      { at: 6000 + BLINK_CLOSE_MS + BLINK_HOLD_MS, phase: 'opening' }, // 6130
      { at: 6000 + BLINK_TOTAL_MS, phase: 'open' }, // 6260
    ]);
  });

  it('re-arms with a FRESH interval, so blinks never land on a grid', () => {
    const randoms = [0.0, 0.9, 1.0, 0.9, 0.25, 0.9];
    const { clock, phases, scheduler } = setup(randoms);
    scheduler.start();
    clock.advance(60_000);

    const starts = phases.filter((p) => p.phase === 'closing').map((p) => p.at);
    const gaps = starts.slice(1).map((s, i) => s - starts[i]);
    // Distinct gaps: the shipped setInterval produced identical ones forever.
    expect(new Set(gaps).size).toBeGreaterThan(1);
    expect(starts[0]).toBe(4200); // r=0.0
    expect(starts[1]).toBe(4200 + BLINK_TOTAL_MS + 7800); // r=1.0
  });

  it('fires a second blink 180-260 ms after the first finishes, then never a third', () => {
    // interval, double? (yes), gap, double? (no, from the second blink we
    // pass canDouble=false so it is not even asked), next interval
    const { clock, phases, scheduler } = setup([0.5, 0.05, 0.0, 0.5]);
    scheduler.start();
    clock.advance(20_000);

    const starts = phases.filter((p) => p.phase === 'closing').map((p) => p.at);
    expect(starts[0]).toBe(6000);
    expect(starts[1]).toBe(6000 + BLINK_TOTAL_MS + 180); // 6440
    // The third blink is a normal interval away, not another double.
    expect(starts[2]! - (starts[1]! + BLINK_TOTAL_MS)).toBeGreaterThanOrEqual(BLINK_MIN_MS);
  });

  it('SUPPRESSES without restarting — the phase never resets on state change', () => {
    // The regression test for the defect this workstream exists for. Every
    // draw is 0.5, so every interval is exactly 6000 ms and no blink doubles;
    // that makes the schedule readable by eye.
    const { clock, phases, scheduler } = setup([0.5]);
    scheduler.start();

    clock.advance(3000); // mid-interval: a dictation starts
    scheduler.setSuppressed(true);
    clock.advance(3000); // the blink due at 6000 is SKIPPED, and re-armed for 12000
    expect(phases.filter((p) => p.phase === 'closing')).toHaveLength(0);

    clock.advance(1000); // t=7000: the dictation ends
    scheduler.setSuppressed(false);

    clock.advance(6000); // t=13000, past both candidate landing times
    const first = phases.find((p) => p.phase === 'closing');
    expect(first).toBeDefined();
    // On the old rhythm, which never stopped underneath the suppression.
    expect(first!.at).toBe(12_000);
    // NOT one fresh interval after the state change, which is what the
    // shipped setInterval teardown/recreate produced — and what made the
    // first blink after every dictation land at exactly the same offset.
    expect(first!.at).not.toBe(7000 + 6000);
  });

  it('opens the eyes immediately when suppressed mid-blink', () => {
    const { clock, phases, scheduler } = setup([0.5, 0.9]);
    scheduler.start();
    clock.advance(6000 + 20); // 20 ms into the close
    expect(phases.at(-1)!.phase).toBe('closing');
    scheduler.setSuppressed(true);
    expect(phases.at(-1)!.phase).toBe('open');
  });

  it('stops cleanly and leaves no timers behind', () => {
    const { clock, phases, scheduler } = setup([0.5, 0.9]);
    scheduler.start();
    clock.advance(1000);
    scheduler.stop();
    expect(clock.pendingCount()).toBe(0);
    expect(phases.at(-1)!.phase).toBe('open');
    clock.advance(60_000);
    expect(phases.filter((p) => p.phase === 'closing')).toHaveLength(0);
  });

  it('is idempotent on start', () => {
    const { clock, scheduler } = setup([0.5, 0.9]);
    scheduler.start();
    scheduler.start();
    expect(clock.pendingCount()).toBe(1);
  });

  it('defaults to Math.random and the ambient timers when nothing is injected', () => {
    vi.useFakeTimers();
    const phases: BlinkPhase[] = [];
    const scheduler = createBlinkScheduler({ onPhase: (p) => phases.push(p) });
    scheduler.start();
    vi.advanceTimersByTime(8000);
    expect(phases).toContain('closing');
    scheduler.stop();
    vi.useRealTimers();
  });
});
