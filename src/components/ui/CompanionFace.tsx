// TTP - Talk To Paste
// The pill's face. Two marks, a clock each, and five ways of drawing them.

import { useEffect, useId, useRef, useState, type CSSProperties } from 'react';
import {
  BREATH_ENABLED,
  DEFAULT_FACE_VARIETY,
  breathPhaseOffsetMs,
  createBlinkScheduler,
  cssTransition,
  eyePath,
  faceVariety,
  transitionFor,
  type BlinkPhase,
  type BlinkScheduler,
  type FaceState,
  type FaceVariety,
  type FaceVarietyId,
  type Transition,
} from './companion-timing';

/**
 * The Companion's face.
 *
 * **Aliveness is timing; charm is drawing.** `docs/companion-faces-design.md`
 * §3.1 argues the first half and dismisses the second — sixteen pixels cannot
 * hold an expression, so put everything into the milliseconds. The first claim
 * is right and this file keeps every one of its numbers. The second is not:
 * sixteen pixels cannot hold an *expression*, but they hold proportion,
 * spacing, terminal softness and the shape of a closed lid perfectly well, and
 * those are what separate a face you like from a face that is merely correct.
 * The whole of that argument, and what was actually rendered and looked at to
 * settle it, is in the `FACE_VARIETIES` comment in `companion-timing.ts`.
 *
 * Rules it must never break:
 *   - **It never demands anything.** Nothing decays, nothing needs feeding,
 *     nothing is ever sad that you did not dictate today. And no reward for
 *     frequency either — that is the same obligation with the sign flipped.
 *     The check to run against any future proposal: if the face's behaviour
 *     depends on anything other than the current dictation state, it is out.
 *   - **Reduced motion removes transitions and loops. It keeps states.** The
 *     face still narrows while thinking and on error, because those are
 *     meaning, not motion. The resolve beat does not run at all — it is pure
 *     animation with no state underneath it.
 *   - It is decorative, so it is aria-hidden. The pill already announces its
 *     state through the live region around it; a face that also spoke would
 *     make a screen reader read every blink.
 *
 * There is deliberately **no mouth**, and no gaze. The manual says the animal
 * has "no discernible front"; a smile is an emotional claim from a character
 * whose entire register is having no opinion; and a punched-out catchlight,
 * which was the first thing tried, reads at every size as a pupil looking
 * somewhere, which is a demand. (§3.6.)
 *
 * Colours come from the theme's custom properties, never from a literal, so
 * the face restyles with the rest of the app. Every one carries a fallback, so
 * a theme that fails to load leaves the face visible rather than invisible.
 */

/**
 * The colour of the marks.
 *
 * `--ttp-ink` rather than `--ttp-accent`, deliberately. The theme layer
 * resolves `--ttp-ink` per context — in the floating-bar window it is the
 * coat's pill ink, a light warm or cool mark chosen to stay legible on the
 * dark body; in a settings panel it is the panel's own foreground. Reading it
 * means the face is legible in both places and carries the coat's colour in
 * both, where reading the raw accent would eventually put a dark accent on a
 * dark pill. `--ttp-glow` is not read at all: it resolves to a *translucent*
 * mix, and as a fill it would dim the eye rather than light it — the sheen is
 * built out of opacity instead. Fallbacks all the way down to white, so a coat
 * that fails to load leaves a visible face rather than an invisible one.
 */
const INK = 'var(--ttp-ink, var(--ttp-accent, #ffffff))';

/**
 * The idle breath: a few percent of scaleY on a slow cycle, CSS keyframes,
 * compositor only.
 *
 * **Never a RAF loop.** The idle pill is on screen permanently by default, so
 * anything that animates at idle animates forever, on a laptop. A permanent JS
 * render loop in an always-on-top window would be an unforced error in an app
 * whose whole pitch is that it is polite.
 *
 * **Why the animated element is the HTML wrapper and not the SVG `<g>` the
 * design doc names.** A CSS transform animation is only cheap if the target
 * gets its own compositing layer, and WebKit — which is what WKWebView, and
 * therefore this window, actually runs — does not generally promote SVG child
 * elements to their own layer. A `transform` keyframe on a `<g>` risks falling
 * back to repainting the SVG every frame, forever, inside a `backdrop-blur`
 * always-on-top window. Same numbers, same visual result, different element:
 * scaling the wrapper about its centre scales eye height and leaves the eyes'
 * position where it was, because the eyes sit within a hair of the box's
 * vertical centre.
 *
 * The `prefers-reduced-motion` block states the intent instead of relying on a
 * side effect. `src/index.css:417` clamps every animation in the app to
 * `0.01ms` with `iteration-count: 1`; that happens to remove this breath,
 * because the `animation` shorthand resets `animation-fill-mode` to `none` and
 * the element reverts to its untransformed state once the (now instant)
 * animation ends. But every other `.anim-*` class in that file uses fill mode
 * `both`, and had this one followed suit the clamp would have frozen the face
 * on the 100% keyframe — a permanent squash rather than no breath at all.
 * `animation: none` does not depend on getting that right. (The component also
 * refuses to apply the class when the media query matches; this is the belt to
 * that's braces, and it is the line the tests exercise.)
 */
function breathCss(v: FaceVariety): string {
  if (!v.breath) return '';
  const { amplitude: a, periodMs } = v.breath;
  const name = `ttp-face-breath-${v.id}`;
  return `
@keyframes ${name} {
  0%, 100% { transform: scaleY(${(1 - a).toFixed(3)}); }
  50%      { transform: scaleY(${(1 + a).toFixed(3)}); }
}
.${name} {
  display: block;
  transform-origin: center;
  /* Guarantees the compositor path rather than hoping for it. The layer is a
     16x16 square; the cost of holding it is not the cost we are avoiding. */
  will-change: transform;
  animation: ${name} ${periodMs}ms ease-in-out infinite;
}
@media (prefers-reduced-motion: reduce) {
  .${name} { animation: none !important; will-change: auto; }
}
`;
}

/** Where the resolve beat is: waiting, overshooting, or settling back. */
type ResolveBeat = 'hold' | 'peak' | 'settle';

export interface CompanionFaceProps {
  state: FaceState;
  reducedMotion: boolean;
  /** Which variety to draw. Defaults to `house`, the survivor. */
  variety?: FaceVarietyId;
}

export function CompanionFace({ state, reducedMotion, variety }: CompanionFaceProps) {
  const v = faceVariety(variety ?? DEFAULT_FACE_VARIETY);
  const [blinkPhase, setBlinkPhase] = useState<BlinkPhase>('open');
  const [beat, setBeat] = useState<ResolveBeat>('hold');
  /* `shut`'s one trick: it is closed when nothing is happening, and the whole
     of its personality is the two hundred milliseconds where that stops being
     true. Starts false so a freshly mounted pill is already asleep — waking up
     on launch would be a greeting, and a greeting is a demand. */
  const [awake, setAwake] = useState(false);
  const [anticipating, setAnticipating] = useState(false);
  const schedulerRef = useRef<BlinkScheduler | null>(null);
  const previousState = useRef<FaceState>(state);
  const glossId = useId();

  // Phase randomised once per window, so the breath is never in lockstep with
  // the clock, the caret, or anything else on screen.
  const [breathSeed] = useState(() => Math.random());
  const breathDelayMs = breathPhaseOffsetMs(breathSeed, v.breath?.periodMs ?? 0);

  /* The blink clock. Mounted once and left alone — it is deliberately NOT in
     the state dependency array, because restarting it on every state change is
     exactly the shipped defect: it made the first blink after every dictation
     land at the same offset, and a regularity a person can find is a dead
     illusion. */
  useEffect(() => {
    if (reducedMotion || !v.blink) {
      setBlinkPhase('open');
      return;
    }
    const scheduler = createBlinkScheduler({ onPhase: setBlinkPhase, profile: v.blink });
    schedulerRef.current = scheduler;
    scheduler.start();
    return () => {
      scheduler.stop();
      schedulerRef.current = null;
    };
  }, [reducedMotion, v.blink]);

  // Suppress, never restart.
  useEffect(() => {
    schedulerRef.current?.setSuppressed(state !== 'idle');
  }, [state]);

  /* Going back to sleep, for the varieties that sleep. Late by `sleepDelayMs`
     so it reads as settling rather than as the tail of the resolve beat. */
  useEffect(() => {
    const previous = previousState.current;
    previousState.current = state;
    if (!v.closedAtIdle) return;
    if (state !== 'idle') {
      setAwake(true);
      return;
    }
    if (previous === 'idle') return; // mounted asleep, or already settled
    if (reducedMotion) {
      setAwake(false);
      return;
    }
    const id = window.setTimeout(() => setAwake(false), v.sleepDelayMs);
    return () => window.clearTimeout(id);
  }, [state, v, reducedMotion]);

  /* The anticipation beat, `quick` only: a 50 ms narrowing *before* the eyes
     open. It is the oldest trick in animation and it is what makes fast motion
     read as intentional rather than abrupt. */
  useEffect(() => {
    if (reducedMotion || !v.wakeAnticipationMs || state !== 'listening') {
      setAnticipating(false);
      return;
    }
    setAnticipating(true);
    const id = window.setTimeout(() => setAnticipating(false), v.wakeAnticipationMs);
    return () => window.clearTimeout(id);
  }, [state, v, reducedMotion]);

  /* The beat when the words land. §3.5 rates this the most valuable addition
     in the document, and the app had nothing there. It is late — 130 to 190 ms
     depending on the variety — because it is a reaction to the app's own
     result (Law 3); that delay is what makes it read as the thing *noticing*
     the paste rather than as a status light firing on the same frame. */
  useEffect(() => {
    if (state !== 'resolving') {
      setBeat('hold');
      return;
    }
    if (reducedMotion) {
      // No beat. It is pure animation; there is no state underneath it, so
      // the face simply arrives at its resting height.
      setBeat('settle');
      return;
    }
    const rise = window.setTimeout(() => setBeat('peak'), v.resolve.delayMs);
    const fall = window.setTimeout(
      () => setBeat('settle'),
      v.resolve.delayMs + v.resolve.riseMs,
    );
    return () => {
      window.clearTimeout(rise);
      window.clearTimeout(fall);
    };
  }, [state, reducedMotion, v]);

  const sleeping = v.closedAtIdle && state === 'idle' && !awake;
  const lidsShut = blinkPhase === 'closing' || sleeping;

  // Where the eyes rest, before the eyelids get a say.
  let restHeight: number;
  if (state === 'resolving') {
    restHeight =
      beat === 'hold'
        ? v.height.thinking // hold the previous state through the delay
        : beat === 'peak'
          ? v.resolve.peakHeight
          : v.height.resolving;
  } else if (anticipating) {
    restHeight = v.height.listening - 0.6;
  } else {
    restHeight = v.height[state];
  }

  const height = lidsShut ? v.shutHeight : restHeight;
  const shape = lidsShut ? v.shut : v.open;

  /* Which clock owns this frame. The blink's asymmetry is the reason this is
     not one constant: `house` shuts in 90 ms and opens in 130. Real eyelids
     close faster than they open, and a symmetric transition reads as a slow
     deliberate wink — a communicative gesture, and therefore a demand.
     `drowsy` inverts it on purpose, which is what heavy-lidded means. */
  let transition: string | undefined;
  if (reducedMotion) {
    transition = undefined;
  } else if (blinkPhase === 'closing') {
    transition = `all ${v.blink?.closeMs ?? 90}ms ease-in`;
  } else if (blinkPhase === 'opening') {
    transition = `all ${v.blink?.openMs ?? 130}ms ease-out`;
  } else if (state === 'resolving') {
    // The beat's lateness is owned by the setTimeout above, so the CSS carries
    // duration and easing only — carrying it twice would double the delay and
    // land the beat outside Law 3's band.
    const t: Transition =
      beat === 'settle'
        ? { delay: 0, duration: v.resolve.settleMs || v.resolve.riseMs, easing: 'ease-in-out' }
        : { delay: 0, duration: v.resolve.riseMs, easing: 'ease-out' };
    transition = cssTransition('all', t);
  } else if (state === 'listening') {
    transition = cssTransition('all', v.wake);
  } else if (sleeping) {
    transition = cssTransition('all', { delay: 0, duration: v.sleepMs, easing: 'ease-in-out' });
  } else {
    transition = cssTransition('all', transitionFor(state));
  }

  const breathing = BREATH_ENABLED && !reducedMotion && state === 'idle' && Boolean(v.breath);
  const breathClass = v.breath ? `ttp-face-breath-${v.id}` : '';
  const opacity = v.opacity[state];
  const fill = v.gloss > 0 ? `url(#${cssId(glossId)})` : INK;

  return (
    <span className="shrink-0 flex items-center" aria-hidden data-ttp-face={v.id}>
      {v.breath && <style>{breathCss(v)}</style>}
      <span
        className={breathing ? breathClass : 'block'}
        style={breathing ? { animationDelay: `${breathDelayMs.toFixed(0)}ms` } : undefined}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          {v.gloss > 0 && (
            <defs>
              {/* A sheen, not a highlight. It shades the lower right *down*
                  rather than lighting the upper left up, so the state opacity
                  the design doc specifies stays the brightest the eye ever
                  gets, and it falls in the same direction as the pill's inset
                  top edge, so the two read as one lit object. */}
              <radialGradient id={cssId(glossId)} cx="34%" cy="24%" r="80%">
                <stop offset="0%" stopColor={INK} stopOpacity={1} />
                <stop offset="100%" stopColor={INK} stopOpacity={1 - v.gloss} />
              </radialGradient>
            </defs>
          )}
          {[-1, 1].map((side) => {
            /* The right eye sits a seventh of a unit further out and a seventh
               lower. Below anyone's threshold of noticing, and the difference
               between something drawn and something computed. */
            const cx = 8 + (side * v.spacing) / 2 + (side > 0 ? v.drift.x : 0);
            const cy = v.cy + (side > 0 ? v.drift.y : 0);
            const d = eyePath(cx, cy, shape.width, height, shape.bow, v.squeeze);
            return (
              <path
                key={side}
                d={d}
                fill={fill}
                fillOpacity={opacity}
                data-eye-height={height}
                /* `d` is set twice on purpose. The attribute is what every
                   engine draws from and what the tests read. The CSS property
                   is what makes the shape *interpolate*: where it is supported
                   the eye morphs — the lid sweeping down over a round eye —
                   and where it is not, the CSS declaration is dropped and the
                   drawing cuts between two correct shapes, which is how
                   hand-drawn blinks have always worked. Neither path can
                   produce a broken frame, because every state emits the same
                   sequence of path commands. */
                style={{ d: `path("${d}")`, transition } as CSSProperties}
              />
            );
          })}
        </svg>
      </span>
    </span>
  );
}

/** `useId` returns a value containing `:`, which is not a legal CSS selector
 *  character and breaks `url(#…)` references in some engines. */
function cssId(id: string): string {
  return `ttp-gloss-${id.replace(/[^a-zA-Z0-9_-]/g, '')}`;
}
