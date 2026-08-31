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
  DEFAULT_FACE_VARIETY,
  FACE_VARIETIES,
  FACE_VARIETY_IDS,
  HOUSE_BLINK,
  asFaceVariety,
  breathPhaseOffsetMs,
  createBlinkScheduler,
  cssTransition,
  doubleBlinkGap,
  eyePath,
  faceVariety,
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

  it('is quieter at idle than the shipped face was, and lower-contrast', () => {
    // A knowing deviation from the doc's §3.5, which took idle down to 3.0.
    // What made the shipped face read as punctuation was the *proportion* —
    // narrow, high, tightly spaced — not the size, and shrinking it made a
    // beadier colon rather than a quieter face. So the eye grows a little and
    // the ink gets paid for in opacity and, far more, in the idle pill
    // dropping from 28px tall to 16px.
    expect(EYE_HEIGHT.idle).toBeLessThan(3.5); // the shipped height
    expect(EYE_HEIGHT.idle).toBe(3.3);
    expect(EYE_OPACITY.idle).toBeLessThan(0.9); // the shipped opacity
    expect(EYE_OPACITY.idle).toBe(0.82);
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
    // ±6% of a 3.3px eye is ±0.198px of eye height.
    const amplitudePx = EYE_HEIGHT.idle * BREATH_AMPLITUDE;
    expect(amplitudePx).toBeCloseTo(0.198, 10);

    // Over one period the height goes down, up, and back: 4 x amplitude of
    // total travel. Law 2 says a change of less than ~1px over more than ~2s
    // is texture rather than an event, and events are what steal a saccade.
    const travelPerPeriodPx = 4 * amplitudePx;
    const travelPerTwoSecondsPx = travelPerPeriodPx * (2000 / BREATH_PERIOD_MS);
    expect(travelPerTwoSecondsPx).toBeCloseTo(0.344, 3);
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

/* -- The cast --------------------------------------------------------------- */

describe('the varieties', () => {
  it('is a set of five, complete and in a stable order', () => {
    // M6 fires on *completeness*, not on count: the whole set unlocks at once,
    // so it needs to be finishable, not large. Ids are persisted in settings
    // and must never be renamed — renaming one silently resets someone's
    // choice back to the default.
    expect(FACE_VARIETY_IDS).toEqual(['house', 'shut', 'drowsy', 'quick', 'bead']);
    for (const id of FACE_VARIETY_IDS) expect(FACE_VARIETIES[id].id).toBe(id);
    expect(DEFAULT_FACE_VARIETY).toBe('house');
  });

  it('falls back to the default rather than drawing nothing', () => {
    expect(faceVariety(null).id).toBe('house');
    expect(faceVariety(undefined).id).toBe('house');
    expect(asFaceVariety('drowsy')).toBe('drowsy');
    expect(asFaceVariety('rupert')).toBeNull();
    expect(asFaceVariety(null)).toBeNull();
    expect(asFaceVariety(42)).toBeNull();
  });

  it('leaves `house` exactly as the design doc specified it', () => {
    const house = FACE_VARIETIES.house;
    expect(house.blink).toBe(HOUSE_BLINK);
    expect(house.blink!.minMs).toBe(4200);
    expect(house.blink!.spreadMs).toBe(3600);
    expect(house.blink!.closeMs).toBe(90);
    expect(house.blink!.holdMs).toBe(40);
    expect(house.blink!.openMs).toBe(130);
    expect(house.blink!.chainChance).toBe(0.12);
    expect(house.breath).toEqual({ periodMs: 4600, amplitude: 0.06 });
    expect(house.wake.delay).toBe(0);
    expect(house.wake.duration).toBe(140);
    expect(house.resolve).toEqual({ delayMs: 140, peakHeight: 3.9, riseMs: 90, settleMs: 170 });
  });

  it('is genuinely different in TIMING — the ten-second-recording falsifier', () => {
    // The design doc's own test for the cast: two varieties must be
    // distinguishable in a ten-second silent recording of the idle state. The
    // mechanical half of that is that no two share an idle rhythm.
    const rhythms = FACE_VARIETY_IDS.map((id) => {
      const v = FACE_VARIETIES[id];
      return JSON.stringify([v.blink, v.breath]);
    });
    expect(new Set(rhythms).size).toBe(FACE_VARIETY_IDS.length);
  });

  it('is genuinely different in DRAWING — and this is a knowing deviation', () => {
    // §4.1 asks for the opposite: "take a still screenshot of two varieties —
    // you should NOT be able to tell them apart". That rule cannot survive its
    // own §4.3, where `shut` sits with its eyes closed at idle, and it does
    // not survive the brief either: a picker whose five rows are identical
    // gives the person choosing nothing to choose between. So the drawings
    // differ too, and the recording test above is kept as the stricter one.
    const drawings = FACE_VARIETY_IDS.map((id) => {
      const v = FACE_VARIETIES[id];
      return JSON.stringify([v.open, v.height.idle, v.spacing, v.cy, v.squeeze, v.closedAtIdle]);
    });
    expect(new Set(drawings).size).toBe(FACE_VARIETY_IDS.length);
  });

  it('never lets a variety demand anything', () => {
    for (const id of FACE_VARIETY_IDS) {
      const v = FACE_VARIETIES[id];
      // Everything a variety can express is keyed to the current dictation
      // state and nothing else. No counters, no streaks, no decay, no calendar
      // — and no reward for frequency either, which is the same obligation
      // with the sign flipped and the one that feels like a good idea.
      expect(Object.keys(v.height).sort()).toEqual(
        ['error', 'idle', 'listening', 'resolving', 'thinking'].sort(),
      );
      expect(Object.keys(v.opacity).sort()).toEqual(
        ['error', 'idle', 'listening', 'resolving', 'thinking'].sort(),
      );
    }
  });

  it('keeps every variety quieter while listening than at idle', () => {
    for (const id of FACE_VARIETY_IDS) {
      const v = FACE_VARIETIES[id];
      expect(v.height.listening).toBeLessThanOrEqual(v.height.idle);
      expect(v.opacity.listening).toBeLessThan(v.opacity.idle);
      // Any state that can outlast four seconds holds a stable appearance.
      expect(v.height.thinking).toBeGreaterThan(v.shutHeight);
      // The overshoot stays inside the glyph. It never moves toward the viewer.
      expect(v.resolve.peakHeight).toBeLessThan(6);
      expect(v.resolve.peakHeight).toBeGreaterThanOrEqual(v.height.resolving);
    }
  });

  it('gives `drowsy` the inverted blink that reads as heavy-lidded', () => {
    const d = FACE_VARIETIES.drowsy.blink!;
    expect(d.openMs).toBeGreaterThan(d.closeMs); // opens SLOWER than it closes
    expect(d.chainChance).toBe(0); // never a flurry
    expect(d.minMs).toBe(7000);
    // Late, and late by exactly the same amount every time: a consistent delay
    // reads as character, a variable one reads as jank.
    expect(FACE_VARIETIES.drowsy.wake.delay).toBe(80);
    expect(FACE_VARIETIES.drowsy.resolve.settleMs).toBe(0); // one slow open, no bounce
  });

  it('gives `quick` short blinks, chains, and an anticipation beat', () => {
    const q = FACE_VARIETIES.quick;
    expect(q.blink!.closeMs).toBeLessThan(FACE_VARIETIES.house.blink!.closeMs);
    expect(q.blink!.chainMax).toBe(2); // doubles, and the occasional triple
    expect(q.wakeAnticipationMs).toBe(50);
    expect(q.squeeze).toBeLessThan(1); // taller than it is wide
  });

  it('gives `shut` and `bead` nothing at all to habituate to at idle', () => {
    // `shut` has nothing open to blink; `bead` simply holds still. Both solve
    // the peripheral-vision problem outright rather than mitigating it.
    expect(FACE_VARIETIES.shut.blink).toBeNull();
    expect(FACE_VARIETIES.shut.breath).toBeNull();
    expect(FACE_VARIETIES.shut.closedAtIdle).toBe(true);
    expect(FACE_VARIETIES.bead.breath).toBeNull();
    expect(FACE_VARIETIES.bead.blink!.chainChance).toBeGreaterThan(0.5);
  });

  it('never wakes `shut` up by itself — the sleep is late, not part of the beat', () => {
    expect(FACE_VARIETIES.shut.sleepDelayMs).toBe(400);
    expect(FACE_VARIETIES.shut.sleepDelayMs).toBeGreaterThan(RESOLVE_SETTLE.duration);
    expect(FACE_VARIETIES.shut.wake.duration).toBe(200);
  });

  it('keeps every reaction to the app inside Law 3\'s 120-200 ms band', () => {
    for (const id of FACE_VARIETY_IDS) {
      const d = FACE_VARIETIES[id].resolve.delayMs;
      expect(d).toBeGreaterThanOrEqual(120);
      expect(d).toBeLessThanOrEqual(200);
      // And the reaction to the user's own key is never delayed by more than
      // the one variety that is late on purpose.
      expect(FACE_VARIETIES[id].wake.delay).toBeLessThanOrEqual(80);
    }
  });
});

/* -- The eye ---------------------------------------------------------------- */

/** The sequence of path commands, which is what has to stay constant. */
const commands = (d: string) => d.replace(/[^A-Z]/g, '');

describe('eyePath', () => {
  it('emits the same command structure for every drawing', () => {
    // The precondition for `d` interpolating rather than snapping: same
    // number of segments, same types, same order. If this ever drifts, blinks
    // stop morphing and start cutting, silently.
    const shapes = FACE_VARIETY_IDS.flatMap((id) => {
      const v = FACE_VARIETIES[id];
      return [
        eyePath(4, 8, v.open.width, v.height.idle, v.open.bow, v.squeeze),
        eyePath(4, 8, v.open.width, v.height.thinking, v.open.bow, v.squeeze),
        eyePath(4, 8, v.open.width, v.resolve.peakHeight, v.open.bow, v.squeeze),
        eyePath(4, 8, v.shut.width, v.shutHeight, v.shut.bow, v.squeeze),
      ];
    });
    for (const d of shapes) expect(commands(d)).toBe('MCACAZ');
    expect(new Set(shapes.map(commands)).size).toBe(1);
  });

  it('centres the ink on cy, so bowing an eye does not shift it up the box', () => {
    const flat = eyePath(8, 8, 3.8, 3.3, 0);
    const bowed = eyePath(8, 8, 3.8, 1.0, 0.62);
    for (const d of [flat, bowed]) {
      const ys = [...d.matchAll(/-?\d+(?:\.\d+)?(?=\s|$)/g)]
        .map((m) => Number(m[0]))
        .filter((n) => n > 3 && n < 13);
      const mid = (Math.min(...ys) + Math.max(...ys)) / 2;
      expect(mid).toBeCloseTo(8, 1);
    }
  });

  it('produces a closed lid that actually curves, and an open eye that does not', () => {
    const house = FACE_VARIETIES.house;
    // The single biggest improvement in the drawing: a shut eye rendered as a
    // flat bar reads as an equals sign, and rendered as a shallow arc reads
    // unmistakably as a closed eye. It bows the way a lid bows over a round
    // eyeball, not the way a smile bows, so it is anatomy and not expression.
    expect(house.shut.bow).toBeGreaterThan(0.5);
    expect(house.open.bow).toBeLessThan(0.2);
    expect(eyePath(8, 8, house.shut.width, house.shutHeight, house.shut.bow)).not.toBe(
      eyePath(8, 8, house.shut.width, house.shutHeight, 0),
    );
  });

  it('refuses a crown taller than the span it crosses', () => {
    // An arch that rises further than its own half-length renders as a little
    // hat rather than an eye. Rendered, looked at, and clamped.
    const silly = eyePath(8, 8, 3.0, 3.0, 40);
    const capped = eyePath(8, 8, 3.0, 3.0, 0.33);
    expect(silly).toBe(capped);
    expect(commands(silly)).toBe('MCACAZ');
  });

  it('squeezes horizontally without changing the command structure', () => {
    const wide = eyePath(8, 8, 3.4, 3.4, 0.05, 1);
    const narrow = eyePath(8, 8, 3.4, 3.4, 0.05, 0.76);
    expect(wide).not.toBe(narrow);
    expect(commands(narrow)).toBe('MCACAZ');
  });
});
