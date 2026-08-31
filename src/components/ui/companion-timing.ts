// TTP - Talk To Paste
// The Companion face's clock.
//
// Every number here comes from `docs/companion-faces-design.md` §3 and is
// quoted, not re-derived. It lives in its own module for one reason: the
// design doc's central claim is that **aliveness is timing, not drawing**, so
// the timing is the part that has to be right, and the part that has to be
// testable without a DOM.
//
// Three laws from §3.1 govern everything below:
//   1. Irregularity, but bounded. Perfectly periodic reads as a mechanism,
//      and the viewer *learns* the period, so periodicity gets worse with
//      exposure. Unbounded random reads as broken.
//   2. The peripheral event threshold. Idle motion under ~1px per ~2s is
//      texture; >2px in <400ms is an event that steals a saccade. Idle must
//      live below the event threshold.
//   3. **A reaction to the user's own action is immediate. A reaction to the
//      app's own result is delayed 120-200 ms.** That delay is the whole
//      difference between a status light and a thing that noticed.

/** The five things the face can be. `resolving` is the beat when words land. */
export type FaceState = 'idle' | 'listening' | 'thinking' | 'resolving' | 'error';

/* -- Law 3: who caused it decides when it happens -------------------------- */

/** Delay applied to a reaction caused by the app's own result (§3.1 Law 3). */
export const REACTION_DELAY_MS = 140;

/**
 * How long to wait before reacting.
 *
 * `user`  — the key they pressed. Zero. Any delay here reads as lag.
 * `app`   — the app's own result. 140 ms, so it reads as *noticing* rather
 *           than as a status light firing on the same frame.
 */
export function reactionDelayFor(cause: 'user' | 'app'): number {
  return cause === 'user' ? 0 : REACTION_DELAY_MS;
}

/* -- Geometry, per §3.5 ---------------------------------------------------- */

/** Eye height per state, in viewBox units of the 16x16 box. */
export const EYE_HEIGHT: Record<FaceState, number> = {
  idle: 3.0,       // was 3.5 — at idle the pill is 16px and the face IS the pill
  listening: 3.0,  // was 4.5 — the bars are the honest instrument; get out of their way
  thinking: 2.2,   // was 1 (shut) — narrowed and HELD, see §3.5
  resolving: 3.0,
  error: 2.2,
};

/** Fill opacity per state. Quieter while listening than while idle, on purpose. */
export const EYE_OPACITY: Record<FaceState, number> = {
  idle: 0.85,
  listening: 0.55,
  thinking: 0.85,
  resolving: 0.85,
  error: 0.85,
};

/** Closed-eye height. Not 0: a collapsed rect vanishes, a 1px line reads shut. */
export const EYE_SHUT_HEIGHT = 1.0;

/** Peak of the resolve overshoot. 3.6 - 3.0 = the whole 0.6px expressive budget. */
export const RESOLVE_PEAK_HEIGHT = 3.6;

/* -- Blinking, per §3.3 ---------------------------------------------------- */

export const BLINK_MIN_MS = 4200;
export const BLINK_SPREAD_MS = 3600; // → 4.2-7.8 s, mean ~6.0 s, ≈10 blinks/min
/** Close is FASTER than open. The asymmetry is load-bearing: a symmetric
 *  transition reads as a slow deliberate wink, and a wink is a demand. */
export const BLINK_CLOSE_MS = 90;
export const BLINK_HOLD_MS = 40;
export const BLINK_OPEN_MS = 130;
/** Total wall time of one blink, close through fully open. */
export const BLINK_TOTAL_MS = BLINK_CLOSE_MS + BLINK_HOLD_MS + BLINK_OPEN_MS; // 260

export const DOUBLE_BLINK_CHANCE = 0.12;
export const DOUBLE_BLINK_GAP_MIN_MS = 180;
export const DOUBLE_BLINK_GAP_SPREAD_MS = 80; // → 180-260 ms after the first finishes

/** Bounded irregularity (Law 1). `r` is a uniform [0,1). */
export function nextBlinkDelay(r: number): number {
  return BLINK_MIN_MS + r * BLINK_SPREAD_MS;
}

/** ~12% of blinks come in pairs. The cheapest four lines in the document. */
export function isDoubleBlink(r: number): boolean {
  return r < DOUBLE_BLINK_CHANCE;
}

/** Gap between the first blink finishing and the second starting. */
export function doubleBlinkGap(r: number): number {
  return DOUBLE_BLINK_GAP_MIN_MS + r * DOUBLE_BLINK_GAP_SPREAD_MS;
}

/* -- Breath, per §3.2 ------------------------------------------------------ */

/**
 * The single switch for the idle breath.
 *
 * §3.2 makes shipping it conditional on a measurement: "Measure it with
 * `powermetrics` before it ships. If the breath costs anything measurable, cut
 * it and keep only blinks." That measurement has NOT been taken — it needs a
 * signed build and a machine where powermetrics can run, neither of which the
 * implementation had. Flip this to `false` to take the doc's own cut; nothing
 * else changes, and the blinks — discrete, and idle 99.9% of the time —
 * remain.
 */
export const BREATH_ENABLED = true;

export const BREATH_PERIOD_MS = 4600;
/** ±6% of eye height. On a 3.0 eye that is ±0.18px — deliberately below the
 *  texture threshold. It may be imperceptible; that is an acceptable outcome. */
export const BREATH_AMPLITUDE = 0.06;

/** Phase offset, randomised once per window so it is never in lockstep with
 *  the clock, the caret, or anything else on screen. Applied as a NEGATIVE
 *  animation-delay so the CSS animation starts mid-cycle. */
export function breathPhaseOffsetMs(r: number): number {
  return -r * BREATH_PERIOD_MS;
}

/* -- State transitions, per §3.5 ------------------------------------------- */

export interface Transition {
  /** ms before the change starts. Law 3 lives here. */
  delay: number;
  /** ms the change takes. */
  duration: number;
  /** CSS easing keyword, quoted from the doc's own words. */
  easing: string;
}

/** ms the error pill shakes for (`anim-shake`, index.css). */
export const ERROR_SHAKE_MS = 320;
/** Beat of stillness after the shake before the face acknowledges it. */
export const ERROR_SETTLE_PAUSE_MS = 200;

/**
 * The transition into a given state.
 *
 * Keyed on the destination because that is how §3.5 specifies them.
 *
 * - `listening` is the user's own key press → delay 0 (Law 3).
 * - `error` holds completely still through the 320 ms shake and a further
 *   200 ms, because a still face inside a shaking body is what reads as a
 *   flinch; a face that animates during a shake reads as two animations.
 */
export function transitionFor(state: FaceState): Transition {
  switch (state) {
    case 'listening':
      return { delay: reactionDelayFor('user'), duration: 140, easing: 'ease-out' };
    case 'thinking':
      return { delay: 0, duration: 180, easing: 'ease-in-out' };
    case 'error':
      return {
        delay: ERROR_SHAKE_MS + ERROR_SETTLE_PAUSE_MS,
        duration: 160,
        easing: 'ease-out',
      };
    case 'resolving':
      // The rise of the beat. Late by REACTION_DELAY_MS because the words
      // landing is the app's own result, not the user's action (Law 3).
      return { delay: reactionDelayFor('app'), duration: 90, easing: 'ease-out' };
    case 'idle':
    default:
      return { delay: 0, duration: 140, easing: 'ease-out' };
  }
}

/**
 * Strip the delay from a Transition.
 *
 * The resolve beat's 140 ms lateness is sequenced in JS, because the beat is
 * two steps (rise then settle) and CSS cannot hold the previous height across
 * both. So the CSS must NOT also carry the delay — that would double-count it
 * and land the beat at 280 ms, outside the 120-200 ms band Law 3 specifies.
 */
export function withoutDelay(t: Transition): Transition {
  return { ...t, delay: 0 };
}

/** The fall of the resolve beat: 3.6 → 3.0, and then it stops talking. */
export const RESOLVE_SETTLE: Transition = { delay: 0, duration: 170, easing: 'ease-in-out' };

/** Total wall time of the resolve beat from `stage === 'complete'`. 140+90+170. */
export const RESOLVE_TOTAL_MS =
  REACTION_DELAY_MS + transitionFor('resolving').duration + RESOLVE_SETTLE.duration;

/** Render a Transition as a CSS `transition` shorthand for `property`. */
export function cssTransition(property: string, t: Transition): string {
  return `${property} ${t.duration}ms ${t.easing} ${t.delay}ms`;
}

/* -- The blink scheduler --------------------------------------------------- */

/** What the eyelids are doing. Drives both height and which easing applies. */
export type BlinkPhase = 'open' | 'closing' | 'opening';

export interface BlinkScheduler {
  start(): void;
  stop(): void;
  /**
   * Suppress blinking WITHOUT restarting the schedule.
   *
   * This is the fix for the shipped defect: the old `setInterval` was torn
   * down and recreated on every state change, so the first blink after every
   * dictation landed at exactly 5200 ms. A regularity a person finds, and
   * finding it is the moment the illusion dies.
   */
  setSuppressed(suppressed: boolean): void;
}

export interface BlinkSchedulerOptions {
  onPhase: (phase: BlinkPhase) => void;
  /** Injectable for tests. Defaults to Math.random. */
  random?: () => number;
  /** Injectable for tests / non-DOM environments. */
  setTimer?: (fn: () => void, ms: number) => number;
  clearTimer?: (id: number) => void;
}

export function createBlinkScheduler(options: BlinkSchedulerOptions): BlinkScheduler {
  const random = options.random ?? Math.random;
  const setTimer =
    options.setTimer ?? ((fn: () => void, ms: number) => setTimeout(fn, ms) as unknown as number);
  const clearTimer = options.clearTimer ?? ((id: number) => clearTimeout(id));

  let timer: number | null = null;
  let running = false;
  let suppressed = false;

  const cancel = () => {
    if (timer !== null) {
      clearTimer(timer);
      timer = null;
    }
  };

  const armNext = () => {
    if (!running) return;
    timer = setTimer(fire, nextBlinkDelay(random()));
  };

  /** `canDouble` is false for the second blink of a pair — never a triple. */
  function fire(canDouble = true) {
    if (!running) return;
    if (suppressed) {
      // The clock keeps running; only the blink is skipped. Phase is never
      // reset by a state change, which is the entire point.
      armNext();
      return;
    }
    options.onPhase('closing');
    timer = setTimer(() => {
      options.onPhase('opening');
      timer = setTimer(() => {
        options.onPhase('open');
        if (canDouble && isDoubleBlink(random())) {
          timer = setTimer(() => fire(false), doubleBlinkGap(random()));
        } else {
          armNext();
        }
      }, BLINK_OPEN_MS);
    }, BLINK_CLOSE_MS + BLINK_HOLD_MS);
  }

  return {
    start() {
      if (running) return;
      running = true;
      armNext();
    },
    stop() {
      running = false;
      cancel();
      options.onPhase('open');
    },
    setSuppressed(next: boolean) {
      suppressed = next;
      // If suppression arrives mid-blink, open the eyes now but leave the
      // chain alone — re-arming here would reintroduce the phase reset.
      if (next) options.onPhase('open');
    },
  };
}
