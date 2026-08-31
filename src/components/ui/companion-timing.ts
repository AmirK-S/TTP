// TTP - Talk To Paste
// The Companion face's clock, and the varieties that read it differently.
//
// Most of the numbers here come from `docs/companion-faces-design.md` §3 and
// are quoted, not re-derived. The module exists so that the part of the face
// that has to be right is also the part that is testable without a DOM.
//
// The doc's thesis is that **aliveness is timing, not drawing**, and it is
// right that timing is what makes two dots feel alive. It is not the whole
// story: timing decides whether the thing is alive, and drawing decides
// whether you want it around. So this module now carries both — a `FaceVariety`
// bundles a set of eye *shapes* with a set of *clocks*, and the varieties are
// deliberately different in both. See `FACE_VARIETIES` below.
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

/**
 * Eye thickness per state, in viewBox units of the 16x16 box. `house`'s table,
 * and the default every other variety is measured against.
 *
 * "Height" here means the *thickness of the ink*, not the shape's bounding
 * box: a bowed eye is `height + bow` tall overall. Keeping the name means
 * every number the design doc quotes still means what it said.
 *
 * **Deviation from §3.5, stated out loud.** The doc takes idle from 3.5 down
 * to 3.0 on the grounds that at idle the face *is* the pill. The premise is
 * right and the conclusion went the wrong way: what made the shipped face read
 * as a mechanism was not that the eyes were large, it was that they were small,
 * narrow, high, and tightly spaced — the proportions of a colon, not of a face.
 * Small-and-high reads severe; large-low-and-wide reads young. So idle goes to
 * 3.3, the width grows from 2.2 to 3.7 (§ `FACE_VARIETIES`), and the ink is
 * paid for by dropping opacity 0.85 -> 0.82 and by the idle pill shrinking from
 * 28px to 16px, which is a far larger reduction in how much of the screen the
 * companion occupies than 0.5 units of eye ever was.
 */
export const EYE_HEIGHT: Record<FaceState, number> = {
  idle: 3.3,       // was 3.5 shipped, 3.0 in the doc — see above
  listening: 3.3,  // was 4.5 — the bars are the honest instrument; get out of their way
  thinking: 2.2,   // was 1 (shut) — narrowed and HELD, see §3.5
  resolving: 3.3,
  error: 2.2,
};

/** Fill opacity per state. Quieter while listening than while idle, on purpose. */
export const EYE_OPACITY: Record<FaceState, number> = {
  idle: 0.82,
  listening: 0.55,
  thinking: 0.82,
  resolving: 0.82,
  error: 0.82,
};

/** Closed-eye height. Not 0: a collapsed rect vanishes, a 1px line reads shut. */
export const EYE_SHUT_HEIGHT = 1.0;

/** Peak of the resolve overshoot. 3.9 - 3.3 = the whole 0.6px expressive budget. */
export const RESOLVE_PEAK_HEIGHT = 3.9;

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
export function breathPhaseOffsetMs(r: number, periodMs: number = BREATH_PERIOD_MS): number {
  return -r * periodMs;
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

/**
 * Everything about how one variety blinks.
 *
 * Defaults are `house`'s, which are the design doc's §3.3 numbers verbatim, so
 * a scheduler constructed with no profile behaves exactly as the shipped one.
 */
export interface BlinkProfile {
  /** Shortest gap between blinks, ms. */
  minMs: number;
  /** Width of the random band above `minMs`. Bounded irregularity (Law 1). */
  spreadMs: number;
  closeMs: number;
  holdMs: number;
  openMs: number;
  /** Probability that a blink is followed by another. */
  chainChance: number;
  /** How many EXTRA blinks a chain may add. 1 = doubles only, 2 = triples. */
  chainMax: number;
  gapMinMs: number;
  gapSpreadMs: number;
}

export const HOUSE_BLINK: BlinkProfile = {
  minMs: BLINK_MIN_MS,
  spreadMs: BLINK_SPREAD_MS,
  closeMs: BLINK_CLOSE_MS,
  holdMs: BLINK_HOLD_MS,
  openMs: BLINK_OPEN_MS,
  chainChance: DOUBLE_BLINK_CHANCE,
  chainMax: 1,
  gapMinMs: DOUBLE_BLINK_GAP_MIN_MS,
  gapSpreadMs: DOUBLE_BLINK_GAP_SPREAD_MS,
};

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
  /** Which variety's rhythm to run. Defaults to `house`. */
  profile?: BlinkProfile;
  /** Injectable for tests. Defaults to Math.random. */
  random?: () => number;
  /** Injectable for tests / non-DOM environments. */
  setTimer?: (fn: () => void, ms: number) => number;
  clearTimer?: (id: number) => void;
}

export function createBlinkScheduler(options: BlinkSchedulerOptions): BlinkScheduler {
  const p = options.profile ?? HOUSE_BLINK;
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
    timer = setTimer(fire, p.minMs + random() * p.spreadMs);
  };

  /** `chainLeft` is how many further blinks this burst may still add. */
  function fire(chainLeft = p.chainMax) {
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
        if (chainLeft > 0 && random() < p.chainChance) {
          const gap = p.gapMinMs + random() * p.gapSpreadMs;
          timer = setTimer(() => fire(chainLeft - 1), gap);
        } else {
          armNext();
        }
      }, p.openMs);
    }, p.closeMs + p.holdMs);
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

/* -- The drawing, and the cast --------------------------------------------- *
 *
 * The design doc deliberately underweights drawing: "aliveness is timing, it is
 * not drawing", and "the face is two rounded rectangles, it will stay two
 * rounded rectangles". Half of that is right. Timing is what decides whether a
 * thing is *alive*. It is not what decides whether you *like* it, and the
 * verdict on the shipped face was not that it seemed dead — it was that it had
 * no charm. Two rounded rectangles, 2.2 wide, 3.5 tall, six units apart, dead
 * centre of the box, is the punctuation mark `:` and reads as one.
 *
 * So the rectangles are gone and the primitive below is a shallow arc with
 * round caps, which is one shape family that can be an open eye at one end of
 * its range and a closed eyelid at the other. The levers that actually make a
 * two-mark face endearing rather than blank, in the order they mattered when
 * the candidates were rendered and looked at:
 *
 *   1. **Proportion.** Large, low, wide-set marks read as young. Small, high,
 *      close-set marks read as severe. The shipped face was the second one.
 *   2. **The closed shape.** A shut eye drawn as a flat bar reads as a domino
 *      or an equals sign. Drawn as a shallow arc that crowns in the middle and
 *      falls at the corners — which is what a lid actually does over a round
 *      eye — it reads unmistakably as a closed eye. This was by a distance the
 *      biggest single improvement, and it is the reason the eye is a path.
 *      It is anatomy, not expression: the arc bows the way a lid bows, not the
 *      way a smile bows, so the face still makes no emotional claim.
 *   3. **Softness of the terminals.** Round caps, no corners, no straight runs.
 *   4. **Asymmetry.** `drift` puts the right eye a seventh of a unit further
 *      out and a seventh lower. It is far below the threshold of noticing and
 *      it is the difference between something drawn and something computed.
 *   5. **A sheen.** A radial gradient a shade brighter at the upper left, in
 *      the same direction as the pill's own inset highlight. Invisible at 1x
 *      and the difference between a printed dot and a lit one on a Retina
 *      display. A punched-out catchlight was tried first and rejected: at any
 *      size it reads as a pupil looking away, which is both a gaze and a
 *      demand.
 */

/** The varieties. Plain nouns, in the sound packs' register. */
export type FaceVarietyId = 'house' | 'shut' | 'drowsy' | 'quick' | 'bead';

/** Order matters — it is the order the settings list shows them in. */
export const FACE_VARIETY_IDS: readonly FaceVarietyId[] = [
  'house',
  'shut',
  'drowsy',
  'quick',
  'bead',
] as const;

export const DEFAULT_FACE_VARIETY: FaceVarietyId = 'house';

/** Narrow an untrusted settings value to a variety id. */
export function asFaceVariety(value: unknown): FaceVarietyId | null {
  return typeof value === 'string' && (FACE_VARIETY_IDS as readonly string[]).includes(value)
    ? (value as FaceVarietyId)
    : null;
}

export interface EyeShape {
  /** Total ink width, round caps included. */
  width: number;
  /**
   * How far the crown rises above the corners. 0 is a plain capsule.
   *
   * **Open eyes want zero.** A bow spans only the flat run between the two
   * round caps, and on an open eye that run is a fraction of a unit wide — so
   * even a bow of 0.1 renders as a nick out of the bottom of the shape rather
   * than as an arch. Rendered under a coat at 2x and looked at; it is a
   * visible dent. The bow belongs to the closed lid, where the run is long.
   */
  bow: number;
}

export interface BreathProfile {
  periodMs: number;
  /** Fraction of eye height. */
  amplitude: number;
}

export interface ResolveProfile {
  /** ms after `stage === 'complete'` before anything moves (Law 3). */
  delayMs: number;
  /** Eye thickness at the top of the beat. Equal to the rest height = no overshoot. */
  peakHeight: number;
  riseMs: number;
  /** 0 for a variety whose beat is a single move with nothing to settle. */
  settleMs: number;
}

export interface FaceVariety {
  id: FaceVarietyId;
  /** Ink thickness per state. */
  height: Record<FaceState, number>;
  /** Ink thickness with the lids shut. */
  shutHeight: number;
  open: EyeShape;
  shut: EyeShape;
  /** Centre-to-centre distance between the eyes. */
  spacing: number;
  /** Vertical centre of the eyes inside the 16x16 box. */
  cy: number;
  /** Horizontal squeeze. Below 1 makes an eye taller than it is wide. */
  squeeze: number;
  opacity: Record<FaceState, number>;
  /** Strength of the upper-left sheen. 0 for a flat variety. */
  gloss: number;
  /** How much further out and lower the right eye sits. The hand in the drawing. */
  drift: { x: number; y: number };
  /** null for a variety with nothing to blink. */
  blink: BlinkProfile | null;
  /** null for a variety that holds perfectly still at idle. */
  breath: BreathProfile | null;
  /** Eyes closed when nothing is happening. */
  closedAtIdle: boolean;
  /** ms after returning to idle before a `closedAtIdle` variety shuts again. */
  sleepDelayMs: number;
  sleepMs: number;
  /** Opening on the user's own key press. */
  wake: Transition;
  /** ms of narrowing before the wake — the classic anticipation beat. 0 = none. */
  wakeAnticipationMs: number;
  resolve: ResolveProfile;
}

/** Uniform opacity for every state except `listening`, which always gets out
 *  of the waveform's way. */
function opacities(base: number): Record<FaceState, number> {
  return {
    idle: base,
    listening: EYE_OPACITY.listening,
    thinking: base,
    resolving: base,
    error: base,
  };
}

/** Heights derived from one open thickness, keeping §3.5's proportions. */
function heights(open: number, narrow: number): Record<FaceState, number> {
  return {
    idle: open,
    listening: open,
    thinking: narrow,
    resolving: open,
    error: narrow,
  };
}

export const FACE_VARIETIES: Record<FaceVarietyId, FaceVariety> = {
  /**
   * The survivor, and the default. Round, wide-set, sitting a little low, with
   * a sheen. Nothing it does is memorable, which is the entire specification.
   */
  house: {
    id: 'house',
    height: EYE_HEIGHT,
    shutHeight: EYE_SHUT_HEIGHT,
    open: { width: 3.8, bow: 0 },
    shut: { width: 3.8, bow: 0.62 },
    spacing: 7.2,
    /* 8.65 rather than a round 8.7: at 16 device pixels a 3.3-thick eye
       centred here has its top edge exactly on the boundary of row 7, which
       is one fewer row of grey. Rasterised at 1x and looked at. */
    cy: 8.65,
    squeeze: 1,
    opacity: EYE_OPACITY,
    gloss: 0.12,
    drift: { x: 0.14, y: 0.14 },
    blink: HOUSE_BLINK,
    breath: { periodMs: BREATH_PERIOD_MS, amplitude: BREATH_AMPLITUDE },
    closedAtIdle: false,
    sleepDelayMs: 0,
    sleepMs: 0,
    wake: transitionFor('listening'),
    wakeAnticipationMs: 0,
    resolve: {
      delayMs: REACTION_DELAY_MS,
      peakHeight: RESOLVE_PEAK_HEIGHT,
      riseMs: transitionFor('resolving').duration,
      settleMs: RESOLVE_SETTLE.duration,
    },
  },

  /**
   * Asleep until you need it. Nothing at all happens at idle — no blink, no
   * breath, nothing to habituate to — and the whole character is the ~200 ms
   * where it opens on your key and the slow close four hundred milliseconds
   * after it is done.
   */
  shut: {
    id: 'shut',
    height: EYE_HEIGHT,
    shutHeight: 1.05,
    open: { width: 3.8, bow: 0 },
    shut: { width: 4.0, bow: 0.74 },
    spacing: 7.2,
    cy: 8.65,
    squeeze: 1,
    opacity: EYE_OPACITY,
    gloss: 0.12,
    drift: { x: 0.14, y: 0.14 },
    blink: null,
    breath: null,
    closedAtIdle: true,
    sleepDelayMs: 400,
    sleepMs: 260,
    wake: { delay: 0, duration: 200, easing: 'ease-out' },
    wakeAnticipationMs: 0,
    resolve: {
      delayMs: REACTION_DELAY_MS,
      peakHeight: RESOLVE_PEAK_HEIGHT,
      riseMs: 90,
      settleMs: 170,
    },
  },

  /**
   * Heavy. Wide, flat, low-slung eyes at three quarters opacity, a long slow
   * breath, and a blink that opens more slowly than it closes — the inversion
   * of `house`, and exactly what reads as heavy-lidded. Its 80 ms late wake is
   * constant to the millisecond, because a consistent delay reads as character
   * and a variable one reads as jank.
   */
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
      spreadMs: 6000, // 7-13 s
      closeMs: 140,
      holdMs: 60,
      openMs: 260, // opens SLOWER than it closes
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
    // A single slow open, no overshoot. It does not have it in it to bounce.
    resolve: { delayMs: 190, peakHeight: 2.1, riseMs: 380, settleMs: 0 },
  },

  /**
   * Awake. Tall narrow eyes set close and high, at nine tenths opacity, short
   * sharp blinks that often come in twos and threes, and an anticipation beat:
   * it narrows for 50 ms before it opens, which is what makes fast motion read
   * as intentional rather than abrupt. Bad company for long-form writing, and
   * the description string says so.
   */
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
      spreadMs: 2300, // 3.2-5.5 s
      closeMs: 70,
      holdMs: 20,
      openMs: 90,
      chainChance: 0.25,
      chainMax: 2, // doubles, and the occasional triple
      gapMinMs: 120,
      gapSpreadMs: 80,
    },
    breath: { periodMs: 3800, amplitude: 0.045 },
    closedAtIdle: false,
    sleepDelayMs: 0,
    sleepMs: 0,
    wake: { delay: 0, duration: 90, easing: 'ease-out' },
    wakeAnticipationMs: 50,
    resolve: { delayMs: 130, peakHeight: 4.2, riseMs: 70, settleMs: 110 },
  },

  /**
   * The smallest thing that is still a face. Two hard little dots set as wide
   * as the box allows, at full opacity and with no sheen and no breath — it
   * does not move at all for ten seconds at a stretch, and then blinks twice
   * in a fifth of a second and goes still again.
   */
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
      spreadMs: 5200, // 5.6-10.8 s — rarely, but then a flurry
      closeMs: 60,
      holdMs: 25,
      openMs: 80,
      chainChance: 0.55,
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

export function faceVariety(id: FaceVarietyId | null | undefined): FaceVariety {
  return FACE_VARIETIES[id ?? DEFAULT_FACE_VARIETY] ?? FACE_VARIETIES[DEFAULT_FACE_VARIETY];
}

/* -- The eye ---------------------------------------------------------------- */

/**
 * The outline of one eye: a shallow arc of thickness `height`, bowing up by
 * `bow` at its crown, with semicircular caps at each end.
 *
 * At `bow = 0.1` and a thickness close to the width it is a soft round eye. At
 * `bow = 0.62` and a thickness of 1 it is a closed lid. There is one shape
 * family, and every state emits the SAME sequence of path commands
 * (`M C A C A Z`), which matters: browsers that interpolate the CSS `d`
 * property morph one drawing into the next, and browsers that do not fall back
 * to a hard cut between two correct drawings — which is how hand-drawn blinks
 * have always been done anyway. Neither path can produce a broken shape.
 *
 * `cy` is where the *ink* is centred, not where the arc's corners sit, so
 * bowing an eye does not shift it up the box.
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
  /* A crown that rises further than the span it crosses reads as a pinch — a
     little hat, not an eye. Rendered and looked at; this cap is where it stops
     happening. */
  const b = Math.max(0, Math.min(bow, a * 0.55 + t * 0.22));
  const k = b * (4 / 3); // cubic control offset that peaks at exactly b
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
