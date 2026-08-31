// TTP - Talk To Paste
// The pill's face. Two rounded rectangles and a clock.

import { useEffect, useRef, useState } from 'react';
import {
  BLINK_CLOSE_MS,
  BLINK_OPEN_MS,
  BREATH_AMPLITUDE,
  BREATH_ENABLED,
  BREATH_PERIOD_MS,
  EYE_HEIGHT,
  EYE_OPACITY,
  EYE_SHUT_HEIGHT,
  REACTION_DELAY_MS,
  RESOLVE_PEAK_HEIGHT,
  RESOLVE_SETTLE,
  breathPhaseOffsetMs,
  createBlinkScheduler,
  cssTransition,
  transitionFor,
  withoutDelay,
  type BlinkPhase,
  type BlinkScheduler,
  type FaceState,
} from './companion-timing';

/**
 * The Companion's face.
 *
 * **Aliveness is timing. It is not drawing.** (docs/companion-faces-design.md
 * §3.1.) Sixteen device pixels cannot hold an expression, and every attempt to
 * put one there produces clip art. What sixteen pixels *can* hold is the
 * difference between 90 ms and 260 ms, and that difference is the whole
 * feature. So this file stays two rounded rects for good, and everything
 * interesting about it lives in `companion-timing.ts`.
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
 * There is deliberately **no mouth**. The manual says the animal has "no
 * discernible front"; a smile is an emotional claim from a character whose
 * entire register is having no opinion; and a 4.8px curve at 1.1 stroke is
 * three device pixels of smudge on a non-Retina display. (§3.6.)
 *
 * The single-face seam: everything a variety would change is a constant in
 * `companion-timing.ts`. That is as far as it goes on purpose — the design
 * doc is explicit that a roster before one face has survived normal use is
 * the riskiest move available.
 */

/**
 * The idle breath: ±6% scaleY, 4600 ms, CSS keyframes, compositor only.
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
 * the eyes sit on the vertical centre of the 16x16 box, which is also the
 * wrapper's centre, so scaling the wrapper about its centre scales eye height
 * and leaves eye position exactly where it was.
 *
 * The `prefers-reduced-motion` block states the intent instead of relying on a
 * side effect. `src/index.css:417` clamps every animation in the app to
 * `0.01ms` with `iteration-count: 1`; that happens to remove this breath,
 * because the `animation` shorthand resets `animation-fill-mode` to `none` and
 * the element reverts to its untransformed state once the (now instant)
 * animation ends. But every other `.anim-*` class in that file uses fill mode
 * `both`, and had this one followed suit the clamp would have frozen the face
 * on the 100% keyframe — a permanent 6% squash rather than no breath at all.
 * `animation: none` does not depend on getting that right. (The component also
 * refuses to apply the class when the media query matches; this is the belt to
 * that's braces, and it is the line the tests exercise.)
 */
const BREATH_CSS = `
@keyframes ttp-face-breath {
  0%, 100% { transform: scaleY(${(1 - BREATH_AMPLITUDE).toFixed(2)}); }
  50%      { transform: scaleY(${(1 + BREATH_AMPLITUDE).toFixed(2)}); }
}
.ttp-face-breath {
  display: block;
  transform-origin: center;
  /* Guarantees the compositor path rather than hoping for it. The layer is a
     16x16 square; the cost of holding it is not the cost we are avoiding. */
  will-change: transform;
  animation: ttp-face-breath ${BREATH_PERIOD_MS}ms ease-in-out infinite;
}
@media (prefers-reduced-motion: reduce) {
  .ttp-face-breath { animation: none !important; will-change: auto; }
}
`;

/** Where the resolve beat is: waiting, overshooting, or settling back. */
type ResolveBeat = 'hold' | 'peak' | 'settle';

export interface CompanionFaceProps {
  state: FaceState;
  reducedMotion: boolean;
}

export function CompanionFace({ state, reducedMotion }: CompanionFaceProps) {
  const [blinkPhase, setBlinkPhase] = useState<BlinkPhase>('open');
  const [beat, setBeat] = useState<ResolveBeat>('hold');
  const schedulerRef = useRef<BlinkScheduler | null>(null);

  // Phase randomised once per window, so the breath is never in lockstep with
  // the clock, the caret, or anything else on screen.
  const [breathDelayMs] = useState(() => breathPhaseOffsetMs(Math.random()));

  /* The blink clock. Mounted once and left alone — it is deliberately NOT in
     the state dependency array, because restarting it on every state change is
     exactly the shipped defect: it made the first blink after every dictation
     land at the same offset, and a regularity a person can find is a dead
     illusion. */
  useEffect(() => {
    if (reducedMotion) {
      setBlinkPhase('open');
      return;
    }
    const scheduler = createBlinkScheduler({ onPhase: setBlinkPhase });
    schedulerRef.current = scheduler;
    scheduler.start();
    return () => {
      scheduler.stop();
      schedulerRef.current = null;
    };
  }, [reducedMotion]);

  // Suppress, never restart.
  useEffect(() => {
    schedulerRef.current?.setSuppressed(state !== 'idle');
  }, [state]);

  /* The beat when the words land. §3.5 rates this the most valuable addition
     in the document, and the app currently has nothing there. It is late by
     140 ms because it is a reaction to the app's own result (Law 3); that
     delay is what makes it read as the thing *noticing* the paste rather than
     as a status light firing on the same frame. */
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
    const rise = window.setTimeout(() => setBeat('peak'), REACTION_DELAY_MS);
    const fall = window.setTimeout(
      () => setBeat('settle'),
      REACTION_DELAY_MS + transitionFor('resolving').duration,
    );
    return () => {
      window.clearTimeout(rise);
      window.clearTimeout(fall);
    };
  }, [state, reducedMotion]);

  // Where the eyes rest, before the eyelids get a say.
  const restHeight =
    state === 'resolving'
      ? beat === 'hold'
        ? EYE_HEIGHT.thinking // hold the previous state through the 140 ms delay
        : beat === 'peak'
          ? RESOLVE_PEAK_HEIGHT
          : EYE_HEIGHT.resolving
      : EYE_HEIGHT[state];

  const eyeHeight = blinkPhase === 'closing' ? EYE_SHUT_HEIGHT : restHeight;

  /* Which clock owns this frame. The blink's asymmetry is the reason this is
     not one constant: 90 ms accelerating shut, 130 ms decelerating open. Real
     eyelids close faster than they open, and a symmetric transition reads as a
     slow deliberate wink — a communicative gesture, and therefore a demand.
     `all` rather than a property list because the animated values are SVG
     geometry presentation attributes, which is what the shipped face already
     relied on. */
  const transition = reducedMotion
    ? undefined
    : blinkPhase === 'closing'
      ? `all ${BLINK_CLOSE_MS}ms ease-in`
      : blinkPhase === 'opening'
        ? `all ${BLINK_OPEN_MS}ms ease-out`
        : state === 'resolving'
          ? // The beat's 140 ms lateness is owned by the setTimeout above, so
            // the CSS carries duration and easing only — carrying it twice
            // would land the beat at 280 ms, outside Law 3's band.
            cssTransition(
              'all',
              beat === 'settle' ? RESOLVE_SETTLE : withoutDelay(transitionFor('resolving')),
            )
          : cssTransition('all', transitionFor(state));

  const breathing = BREATH_ENABLED && !reducedMotion && state === 'idle';

  return (
    <span className="shrink-0 flex items-center" aria-hidden>
      <style>{BREATH_CSS}</style>
      <span
        className={breathing ? 'ttp-face-breath' : 'block'}
        style={breathing ? { animationDelay: `${breathDelayMs.toFixed(0)}ms` } : undefined}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          {[5, 11].map((cx) => (
            <rect
              key={cx}
              x={cx - 1.1}
              y={8 - eyeHeight / 2}
              width={2.2}
              height={eyeHeight}
              rx={1.1}
              className="fill-white"
              style={{ fillOpacity: EYE_OPACITY[state], transition }}
            />
          ))}
        </svg>
      </span>
    </span>
  );
}
