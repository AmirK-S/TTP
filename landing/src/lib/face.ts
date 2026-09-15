/* ----------------------------------------------------------------------------
   The Companion's face, on the web.

   A port of `src/components/ui/companion-timing.ts` and `CompanionFace.tsx`
   with React removed. Every number is quoted from the app, not re-derived,
   because the whole claim the page makes about the varieties is that they
   differ in *timing* — so a face on the marketing page that blinks on some
   made-up clock would be lying about the product.

   Three laws it keeps (docs/companion-faces-design.md §3.1):
     1. Irregularity, but bounded. Anything perfectly periodic is a mechanism,
        and the viewer learns the period.
     2. Idle motion stays below the peripheral-event threshold.
     3. A reaction to the user's own action is immediate; a reaction to the
        app's own result is 120-200 ms late. That delay is the entire
        difference between a status light and a thing that noticed.

   And two constraints from the brief:
     - never a RAF loop. These faces sit on a page that may be left open.
       Blinking is a chain of setTimeouts and CSS transitions; the idle breath
       is a CSS keyframe on the compositor.
     - `prefers-reduced-motion` removes transitions and loops but keeps
        states, because narrowing while thinking is meaning, not motion.
   ------------------------------------------------------------------------- */

export type FaceState = 'idle' | 'listening' | 'thinking' | 'resolving' | 'error';
export type FaceVarietyId = 'house' | 'shut' | 'drowsy' | 'quick' | 'bead';

export const FACE_VARIETY_IDS: readonly FaceVarietyId[] = [
  'house',
  'shut',
  'drowsy',
  'quick',
  'bead',
];

/** Delay applied to a reaction caused by the app's own result (Law 3). */
const REACTION_DELAY_MS = 140;

interface BlinkProfile {
  minMs: number;
  spreadMs: number;
  closeMs: number;
  holdMs: number;
  openMs: number;
  chainChance: number;
  chainMax: number;
  gapMinMs: number;
  gapSpreadMs: number;
}

interface EyeShape {
  width: number;
  bow: number;
}

export interface FaceVariety {
  id: FaceVarietyId;
  height: Record<FaceState, number>;
  shutHeight: number;
  open: EyeShape;
  shut: EyeShape;
  spacing: number;
  cy: number;
  squeeze: number;
  opacity: Record<FaceState, number>;
  gloss: number;
  drift: { x: number; y: number };
  blink: BlinkProfile | null;
  breath: { periodMs: number; amplitude: number } | null;
  closedAtIdle: boolean;
  sleepDelayMs: number;
  sleepMs: number;
  wake: { delay: number; duration: number; easing: string };
  wakeAnticipationMs: number;
  resolve: { delayMs: number; peakHeight: number; riseMs: number; settleMs: number };
}

const HOUSE_BLINK: BlinkProfile = {
  minMs: 4200,
  spreadMs: 3600, // 4.2-7.8 s, ~10 blinks a minute
  closeMs: 90,
  holdMs: 40,
  openMs: 130, // closes faster than it opens; the asymmetry is the whole
  chainChance: 0.12, //   difference between a blink and a wink
  chainMax: 1,
  gapMinMs: 180,
  gapSpreadMs: 80,
};

const HOUSE_HEIGHTS: Record<FaceState, number> = {
  idle: 3.3,
  listening: 3.3,
  thinking: 2.2,
  resolving: 3.3,
  error: 2.2,
};

const HOUSE_OPACITY: Record<FaceState, number> = {
  idle: 0.82,
  listening: 0.55, // quieter while listening; the bars are the instrument
  thinking: 0.82,
  resolving: 0.82,
  error: 0.82,
};

function opacities(base: number): Record<FaceState, number> {
  return { idle: base, listening: 0.55, thinking: base, resolving: base, error: base };
}

function heights(open: number, narrow: number): Record<FaceState, number> {
  return { idle: open, listening: open, thinking: narrow, resolving: open, error: narrow };
}

export const FACE_VARIETIES: Record<FaceVarietyId, FaceVariety> = {
  house: {
    id: 'house',
    height: HOUSE_HEIGHTS,
    shutHeight: 1.0,
    open: { width: 3.8, bow: 0 },
    shut: { width: 3.8, bow: 0.62 },
    spacing: 7.2,
    cy: 8.65,
    squeeze: 1,
    opacity: HOUSE_OPACITY,
    gloss: 0.12,
    drift: { x: 0.14, y: 0.14 },
    blink: HOUSE_BLINK,
    breath: { periodMs: 4600, amplitude: 0.06 },
    closedAtIdle: false,
    sleepDelayMs: 0,
    sleepMs: 0,
    wake: { delay: 0, duration: 140, easing: 'ease-out' },
    wakeAnticipationMs: 0,
    resolve: { delayMs: REACTION_DELAY_MS, peakHeight: 3.9, riseMs: 90, settleMs: 170 },
  },

  shut: {
    id: 'shut',
    height: HOUSE_HEIGHTS,
    shutHeight: 1.05,
    open: { width: 3.8, bow: 0 },
    shut: { width: 4.0, bow: 0.74 },
    spacing: 7.2,
    cy: 8.65,
    squeeze: 1,
    opacity: HOUSE_OPACITY,
    gloss: 0.12,
    drift: { x: 0.14, y: 0.14 },
    blink: null,
    breath: null,
    closedAtIdle: true,
    sleepDelayMs: 400,
    sleepMs: 260,
    wake: { delay: 0, duration: 200, easing: 'ease-out' },
    wakeAnticipationMs: 0,
    resolve: { delayMs: REACTION_DELAY_MS, peakHeight: 3.9, riseMs: 90, settleMs: 170 },
  },

  drowsy: {
    id: 'drowsy',
    height: heights(2.1, 1.6),
    shutHeight: 1.0,
    open: { width: 4.4, bow: 0.18 },
    shut: { width: 4.4, bow: 0.8 },
    spacing: 7.5,
    cy: 9.0,
    squeeze: 1,
    opacity: opacities(0.74),
    gloss: 0,
    drift: { x: 0.1, y: 0.2 },
    blink: {
      minMs: 7000,
      spreadMs: 6000,
      closeMs: 140,
      holdMs: 60,
      openMs: 260, // opens SLOWER than it closes: heavy-lidded
      chainChance: 0,
      chainMax: 0,
      gapMinMs: 0,
      gapSpreadMs: 0,
    },
    breath: { periodMs: 6500, amplitude: 0.1 },
    closedAtIdle: false,
    sleepDelayMs: 0,
    sleepMs: 0,
    wake: { delay: 80, duration: 260, easing: 'ease-out' },
    wakeAnticipationMs: 0,
    resolve: { delayMs: 190, peakHeight: 2.1, riseMs: 380, settleMs: 0 },
  },

  quick: {
    id: 'quick',
    height: heights(3.4, 2.4),
    shutHeight: 0.9,
    open: { width: 3.4, bow: 0 },
    shut: { width: 3.3, bow: 0.42 },
    spacing: 6.4,
    cy: 8.2,
    squeeze: 0.76,
    opacity: opacities(0.9),
    gloss: 0.18,
    drift: { x: 0.1, y: 0.08 },
    blink: {
      minMs: 3200,
      spreadMs: 2300,
      closeMs: 70,
      holdMs: 20,
      openMs: 90,
      chainChance: 0.25,
      chainMax: 2,
      gapMinMs: 120,
      gapSpreadMs: 80,
    },
    breath: { periodMs: 3800, amplitude: 0.045 },
    closedAtIdle: false,
    sleepDelayMs: 0,
    sleepMs: 0,
    wake: { delay: 0, duration: 90, easing: 'ease-out' },
    wakeAnticipationMs: 50, // narrows before it opens: the anticipation beat
    resolve: { delayMs: 130, peakHeight: 4.2, riseMs: 70, settleMs: 110 },
  },

  bead: {
    id: 'bead',
    height: heights(2.3, 1.7),
    shutHeight: 0.9,
    open: { width: 2.5, bow: 0 },
    shut: { width: 2.8, bow: 0.35 },
    spacing: 7.9,
    cy: 8.5,
    squeeze: 1,
    opacity: opacities(0.95),
    gloss: 0,
    drift: { x: 0.14, y: 0.1 },
    blink: {
      minMs: 5600,
      spreadMs: 5200,
      closeMs: 60,
      holdMs: 25,
      openMs: 80,
      chainChance: 0.55, // rarely, and then a flurry
      chainMax: 2,
      gapMinMs: 110,
      gapSpreadMs: 60,
    },
    breath: null,
    closedAtIdle: false,
    sleepDelayMs: 0,
    sleepMs: 0,
    wake: { delay: 0, duration: 110, easing: 'ease-out' },
    wakeAnticipationMs: 0,
    resolve: { delayMs: REACTION_DELAY_MS, peakHeight: 2.8, riseMs: 80, settleMs: 120 },
  },
};

/**
 * The outline of one eye: a shallow arc of thickness `height`, bowing up by
 * `bow` at its crown, with semicircular caps at each end.
 *
 * Every state emits the same sequence of path commands (`M C A C A Z`), so an
 * engine that interpolates the CSS `d` property morphs one drawing into the
 * next and an engine that does not cuts between two correct drawings. Neither
 * can produce a broken shape.
 */
export function eyePath(
  cx: number,
  cy: number,
  width: number,
  height: number,
  bow: number,
  squeeze = 1,
): string {
  const t = height / 2;
  const a = Math.max(0, (width - height) / 2);
  // A crown rising further than the span it crosses reads as a little hat.
  const b = Math.max(0, Math.min(bow, a * 0.55 + t * 0.22));
  const k = b * (4 / 3);
  const y = cy + b / 2;
  const n = (v: number) => Number(v.toFixed(3));
  const X = (dx: number) => n(cx + dx * squeeze);
  const rxCap = n(t * squeeze);
  return [
    `M${X(-a)} ${n(y - t)}`,
    `C${X(-a / 3)} ${n(y - t - k)} ${X(a / 3)} ${n(y - t - k)} ${X(a)} ${n(y - t)}`,
    `A${rxCap} ${n(t)} 0 0 1 ${X(a)} ${n(y + t)}`,
    `C${X(a / 3)} ${n(y + t - k)} ${X(-a / 3)} ${n(y + t - k)} ${X(-a)} ${n(y + t)}`,
    `A${rxCap} ${n(t)} 0 0 1 ${X(-a)} ${n(y - t)}`,
    'Z',
  ].join(' ');
}

/** The two eye centres, in viewBox units. The right one drifts a seventh of a
 *  unit further out and lower — below anyone's threshold of noticing, and the
 *  difference between something drawn and something computed. */
export function eyeCentres(v: FaceVariety): Array<{ cx: number; cy: number }> {
  return [-1, 1].map((side) => ({
    cx: 8 + (side * v.spacing) / 2 + (side > 0 ? v.drift.x : 0),
    cy: v.cy + (side > 0 ? v.drift.y : 0),
  }));
}

/* -- the driver ------------------------------------------------------------ */

type BlinkPhase = 'open' | 'closing' | 'opening';

export interface FaceHandle {
  setState(state: FaceState): void;
  destroy(): void;
}

/**
 * Drive one already-rendered face element.
 *
 * `root` must contain two `<path data-eye>` children — the markup is emitted at
 * build time by `CompanionFace.astro`, so the face is correct before any script
 * runs and JS only makes it move.
 */
export function mountFace(root: SVGElement | HTMLElement, id: FaceVarietyId): FaceHandle {
  const v = FACE_VARIETIES[id] ?? FACE_VARIETIES.house;
  const paths = Array.from(root.querySelectorAll<SVGPathElement>('[data-eye]'));
  const centres = eyeCentres(v);
  const reduced =
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  let state: FaceState = 'idle';
  let blinkPhase: BlinkPhase = 'open';
  let beat: 'hold' | 'peak' | 'settle' = 'hold';
  // `shut` starts asleep: waking up on load would be a greeting, and a greeting
  // is a demand.
  let awake = !v.closedAtIdle;
  let anticipating = false;
  const timers = new Set<number>();

  const later = (fn: () => void, ms: number): number => {
    const t = window.setTimeout(() => {
      timers.delete(t);
      fn();
    }, ms);
    timers.add(t);
    return t;
  };

  function draw() {
    const sleeping = v.closedAtIdle && state === 'idle' && !awake;
    const lidsShut = blinkPhase === 'closing' || sleeping;

    let rest: number;
    if (state === 'resolving') {
      rest =
        beat === 'hold'
          ? v.height.thinking // hold the previous state through the delay
          : beat === 'peak'
            ? v.resolve.peakHeight
            : v.height.resolving;
    } else if (anticipating) {
      rest = v.height.listening - 0.6;
    } else {
      rest = v.height[state];
    }

    const height = lidsShut ? v.shutHeight : rest;
    const shape = lidsShut ? v.shut : v.open;

    let transition = '';
    if (reduced) {
      transition = 'none';
    } else if (blinkPhase === 'closing') {
      transition = `${v.blink?.closeMs ?? 90}ms ease-in`;
    } else if (blinkPhase === 'opening') {
      transition = `${v.blink?.openMs ?? 130}ms ease-out`;
    } else if (state === 'resolving') {
      // The lateness lives in the setTimeout below, never here as well —
      // carrying it twice would land the beat outside Law 3's band.
      transition =
        beat === 'settle'
          ? `${v.resolve.settleMs || v.resolve.riseMs}ms ease-in-out`
          : `${v.resolve.riseMs}ms ease-out`;
    } else if (state === 'listening') {
      transition = `${v.wake.duration}ms ${v.wake.easing} ${v.wake.delay}ms`;
    } else if (sleeping) {
      transition = `${v.sleepMs}ms ease-in-out`;
    } else if (state === 'error') {
      // Perfectly still through the shake, and 200 ms after it, before the face
      // acknowledges anything. A still face in a shaking body reads as a flinch.
      transition = `160ms ease-out 520ms`;
    } else {
      transition = `140ms ease-out`;
    }

    paths.forEach((p, i) => {
      const c = centres[i];
      const d = eyePath(c.cx, c.cy, shape.width, height, shape.bow, v.squeeze);
      p.setAttribute('d', d);
      p.style.transitionDuration = transition === 'none' ? '0s' : '';
      p.style.transition = transition === 'none' ? 'none' : `d ${transition}, fill-opacity ${transition}`;
      // Setting `d` twice on purpose: the attribute is what every engine draws
      // from, the CSS property is what makes it interpolate.
      p.style.setProperty('d', `path("${d}")`);
      p.setAttribute('fill-opacity', String(v.opacity[state]));
    });
  }

  /* The blink clock. Started once and never restarted on a state change —
     restarting it is the shipped defect that made the first blink after every
     dictation land at the same offset, and a regularity a person can find is a
     dead illusion. Suppression skips the blink; the clock keeps running. */
  let suppressed = false;
  let running = false;

  function armNext() {
    if (!running || !v.blink) return;
    later(() => fire(v.blink!.chainMax), v.blink.minMs + Math.random() * v.blink.spreadMs);
  }

  function fire(chainLeft: number) {
    if (!running || !v.blink) return;
    if (suppressed) {
      armNext();
      return;
    }
    const b = v.blink;
    blinkPhase = 'closing';
    draw();
    later(() => {
      blinkPhase = 'opening';
      draw();
      later(() => {
        blinkPhase = 'open';
        draw();
        if (chainLeft > 0 && Math.random() < b.chainChance) {
          later(() => fire(chainLeft - 1), b.gapMinMs + Math.random() * b.gapSpreadMs);
        } else {
          armNext();
        }
      }, b.openMs);
    }, b.closeMs + b.holdMs);
  }

  if (!reduced && v.blink) {
    running = true;
    armNext();
  }

  // The idle breath: a CSS keyframe on the wrapper, phase-randomised once so it
  // is never in lockstep with anything else on the page.
  const breathEl = root.querySelector<HTMLElement>('.breath');
  if (breathEl) {
    if (reduced || !v.breath) {
      breathEl.style.animation = 'none';
    } else {
      breathEl.style.setProperty('--breath-period', `${v.breath.periodMs}ms`);
      breathEl.style.animationDelay = `${-Math.random() * v.breath.periodMs}ms`;
    }
  }

  draw();

  return {
    setState(next: FaceState) {
      const previous = state;
      state = next;
      suppressed = next !== 'idle';

      if (v.closedAtIdle) {
        if (next !== 'idle') {
          awake = true;
        } else if (previous !== 'idle') {
          if (reduced) awake = false;
          else later(() => { awake = false; draw(); }, v.sleepDelayMs);
        }
      }

      if (!reduced && v.wakeAnticipationMs && next === 'listening') {
        anticipating = true;
        later(() => { anticipating = false; draw(); }, v.wakeAnticipationMs);
      } else {
        anticipating = false;
      }

      if (next === 'resolving') {
        if (reduced) {
          // No beat. It is pure animation with no state underneath it, so the
          // face simply arrives at its resting height.
          beat = 'settle';
        } else {
          beat = 'hold';
          later(() => { beat = 'peak'; draw(); }, v.resolve.delayMs);
          later(() => { beat = 'settle'; draw(); }, v.resolve.delayMs + v.resolve.riseMs);
        }
      } else {
        beat = 'hold';
      }

      draw();
    },
    destroy() {
      running = false;
      timers.forEach((t) => window.clearTimeout(t));
      timers.clear();
    },
  };
}
