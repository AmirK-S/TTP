// TTP - Talk To Paste
// Tests for the Companion face as rendered.
//
// The timing logic is covered by companion-timing.test.ts against a virtual
// clock. What this file checks is that the component actually *uses* it: that
// the eyes are not shut during transcription, that the beat runs when the
// words land, that reduced motion removes loops and transitions while keeping
// states, and that there is no mouth.
//
// The eyes are `<path>` rather than `<rect>` now — one shape family that is a
// round eye at one end of its range and a closed lid at the other, because a
// shut eye drawn as a flat bar reads as an equals sign. The thickness each
// path was built at is mirrored onto `data-eye-height`, so every assertion
// below still reads the number the design doc specifies.

import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CompanionFace } from './CompanionFace';
import {
  BLINK_CLOSE_MS,
  BLINK_OPEN_MS,
  EYE_HEIGHT,
  EYE_SHUT_HEIGHT,
  REACTION_DELAY_MS,
  RESOLVE_PEAK_HEIGHT,
  BLINK_MIN_MS,
  BLINK_SPREAD_MS,
  FACE_VARIETIES,
  FACE_VARIETY_IDS,
} from './companion-timing';

/** Ink thickness of the two eyes, as the DOM actually has them. */
function eyeHeights(container: HTMLElement): number[] {
  return Array.from(container.querySelectorAll('[data-eye-height]')).map((r) =>
    Number(r.getAttribute('data-eye-height')),
  );
}

function eyes(container: HTMLElement): SVGPathElement[] {
  return Array.from(container.querySelectorAll<SVGPathElement>('path[data-eye-height]'));
}

function eyeTransition(container: HTMLElement): string {
  return (container.querySelector('path') as SVGPathElement).style.transition;
}

describe('CompanionFace', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('has no mouth in any state', () => {
    // Deleted on purpose (§3.6): the manual says the animal has no
    // discernible front, a smile is an emotional claim from a character whose
    // register is having no opinion, and a 1.1-stroke curve is a smudge on a
    // non-Retina display.
    for (const state of ['idle', 'listening', 'thinking', 'resolving', 'error'] as const) {
      const { container, unmount } = render(
        <CompanionFace state={state} reducedMotion={false} />,
      );
      // Two eyes and nothing else. No mouth, no third mark, no pupil.
      expect(eyes(container)).toHaveLength(2);
      expect(
        container.querySelectorAll('path, rect, circle, ellipse, line, polyline, polygon, text'),
      ).toHaveLength(2);
      unmount();
    }
  });

  it('is decorative, so it is hidden from assistive tech', () => {
    const { container } = render(<CompanionFace state="idle" reducedMotion={false} />);
    expect(container.querySelector('[aria-hidden]')).not.toBeNull();
    expect(screen.queryByRole('img')).toBeNull();
  });

  it('narrows and HOLDS while thinking — it never shuts its eyes', () => {
    // The worst-placed behaviour in the shipped face: it went blank for the
    // whole of transcription, which is exactly when the user is looking.
    const { container } = render(<CompanionFace state="thinking" reducedMotion={false} />);
    expect(eyeHeights(container)).toEqual([2.2, 2.2]);

    // Ten seconds of a long transcription: nothing moves at all.
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    expect(eyeHeights(container)).toEqual([2.2, 2.2]);
    expect(eyeHeights(container)[0]).not.toBe(EYE_SHUT_HEIGHT);
  });

  it('gets quieter while listening, not louder', () => {
    const { container } = render(<CompanionFace state="listening" reducedMotion={false} />);
    expect(eyeHeights(container)).toEqual([EYE_HEIGHT.listening, EYE_HEIGHT.listening]);
    expect(container.querySelector('path')!.getAttribute('fill-opacity')).toBe('0.55');
  });

  it('responds to the user immediately — no delay entering listening', () => {
    const { container } = render(<CompanionFace state="listening" reducedMotion={false} />);
    expect(eyeTransition(container)).toBe('all 140ms ease-out 0ms');
  });

  describe('the resolve beat', () => {
    it('waits 140 ms, overshoots, then settles back', () => {
      const { container, rerender } = render(
        <CompanionFace state="thinking" reducedMotion={false} />,
      );
      rerender(<CompanionFace state="resolving" reducedMotion={false} />);

      // Frame zero: nothing. The words have landed; the face has not noticed
      // them yet, and that gap is the whole point.
      expect(eyeHeights(container)).toEqual([2.2, 2.2]);
      act(() => {
        vi.advanceTimersByTime(REACTION_DELAY_MS - 1);
      });
      expect(eyeHeights(container)).toEqual([2.2, 2.2]);

      // 140 ms: the overshoot.
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(eyeHeights(container)).toEqual([RESOLVE_PEAK_HEIGHT, RESOLVE_PEAK_HEIGHT]);
      expect(eyeTransition(container)).toBe('all 90ms ease-out 0ms');

      // +90 ms: the settle, and then it stops talking.
      act(() => {
        vi.advanceTimersByTime(90);
      });
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
      expect(eyeTransition(container)).toBe('all 170ms ease-in-out 0ms');
    });

    it('leaves no residue once the pill returns to idle', () => {
      const { container, rerender } = render(
        <CompanionFace state="resolving" reducedMotion={false} />,
      );
      act(() => {
        vi.advanceTimersByTime(400);
      });
      rerender(<CompanionFace state="idle" reducedMotion={false} />);
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
    });
  });

  describe('blinking', () => {
    it('closes faster than it opens', () => {
      // Draw order: the breath's phase offset, then the blink interval, then
      // the double-blink coin. 0 → the shortest interval; 0.99 → not a double.
      const randoms = [0.5, 0, 0.99];
      let i = 0;
      vi.spyOn(Math, 'random').mockImplementation(
        () => randoms[Math.min(i++, randoms.length - 1)],
      );

      const { container } = render(<CompanionFace state="idle" reducedMotion={false} />);
      act(() => {
        vi.advanceTimersByTime(BLINK_MIN_MS);
      });
      expect(eyeHeights(container)).toEqual([EYE_SHUT_HEIGHT, EYE_SHUT_HEIGHT]);
      expect(eyeTransition(container)).toBe(`all ${BLINK_CLOSE_MS}ms ease-in`);

      act(() => {
        vi.advanceTimersByTime(BLINK_CLOSE_MS + 40);
      });
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
      expect(eyeTransition(container)).toBe(`all ${BLINK_OPEN_MS}ms ease-out`);
      vi.mocked(Math.random).mockRestore();
    });

    it('never blinks outside idle', () => {
      const { container } = render(<CompanionFace state="listening" reducedMotion={false} />);
      act(() => {
        vi.advanceTimersByTime(BLINK_MIN_MS + BLINK_SPREAD_MS + 5000);
      });
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.listening, EYE_HEIGHT.listening]);
    });
  });

  describe('reduced motion', () => {
    it('keeps the states — thinking still narrows, error still narrows', () => {
      // Reduced motion removes transitions and loops. It keeps *meaning*.
      const { container, rerender } = render(
        <CompanionFace state="thinking" reducedMotion />,
      );
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.thinking, EYE_HEIGHT.thinking]);
      rerender(<CompanionFace state="error" reducedMotion />);
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.error, EYE_HEIGHT.error]);
    });

    it('makes every state change an instant cut', () => {
      const { container } = render(<CompanionFace state="listening" reducedMotion />);
      expect(eyeTransition(container)).toBe('');
    });

    it('runs no blink loop and schedules no timers', () => {
      const { container } = render(<CompanionFace state="idle" reducedMotion />);
      expect(vi.getTimerCount()).toBe(0);
      act(() => {
        vi.advanceTimersByTime(60_000);
      });
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
    });

    it('does not run the breath animation', () => {
      const { container } = render(<CompanionFace state="idle" reducedMotion />);
      expect(container.querySelector('.ttp-face-breath-house')).toBeNull();
    });

    it('runs the breath at idle when motion is allowed, and only at idle', () => {
      const { container, rerender } = render(
        <CompanionFace state="idle" reducedMotion={false} />,
      );
      const breath = container.querySelector('.ttp-face-breath-house') as HTMLElement | null;
      expect(breath).not.toBeNull();
      // A negative delay, so the phase is not in lockstep with the clock, the
      // caret, or anything else on screen.
      expect(Number(breath!.style.animationDelay.replace('ms', ''))).toBeLessThanOrEqual(0);

      rerender(<CompanionFace state="thinking" reducedMotion={false} />);
      expect(container.querySelector('.ttp-face-breath-house')).toBeNull();
    });

    it('breathes on an HTML element, never on SVG innards', () => {
      // A transform keyframe on an SVG <g> is not reliably composited in
      // WebKit, and this animation runs forever on battery. It has to be on
      // something the compositor will promote.
      const { container } = render(<CompanionFace state="idle" reducedMotion={false} />);
      const breath = container.querySelector('.ttp-face-breath-house')!;
      expect(breath.tagName.toLowerCase()).toBe('span');
      expect(breath.closest('svg')).toBeNull();
    });

    it('never drives the breath from a JS loop', () => {
      // The idle pill is on screen permanently by default. A RAF loop here
      // would burn battery forever for a decoration.
      const raf = vi.spyOn(window, 'requestAnimationFrame');
      render(<CompanionFace state="idle" reducedMotion={false} />);
      act(() => {
        vi.advanceTimersByTime(30_000);
      });
      expect(raf).not.toHaveBeenCalled();
      raf.mockRestore();
    });

    it('does not run the resolve beat at all — it is pure animation', () => {
      const { container } = render(<CompanionFace state="resolving" reducedMotion />);
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
      expect(vi.getTimerCount()).toBe(0);
    });

    it('cancels a running blink when reduced motion is turned on mid-session', () => {
      const { container, rerender } = render(
        <CompanionFace state="idle" reducedMotion={false} />,
      );
      expect(vi.getTimerCount()).toBeGreaterThan(0);
      rerender(<CompanionFace state="idle" reducedMotion />);
      expect(vi.getTimerCount()).toBe(0);
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
    });
  });

  it('leaves no timers behind on unmount', () => {
    const { unmount } = render(<CompanionFace state="idle" reducedMotion={false} />);
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  /* -- The cast ------------------------------------------------------------ */

  describe('the cast', () => {
    it('draws two eyes and nothing else, whichever variety is on', () => {
      for (const id of FACE_VARIETY_IDS) {
        const { container, unmount } = render(
          <CompanionFace state="idle" reducedMotion variety={id} />,
        );
        expect(eyes(container)).toHaveLength(2);
        expect(container.querySelector('[data-ttp-face]')?.getAttribute('data-ttp-face')).toBe(id);
        unmount();
      }
    });

    it('draws the right eye a hair further out and a hair lower than the left', () => {
      // Subtle asymmetry is what separates something drawn from something
      // computed, and it is well under anyone's threshold of noticing.
      const { container } = render(<CompanionFace state="idle" reducedMotion />);
      const [left, right] = eyes(container).map((e) => e.getAttribute('d')!);
      expect(left).not.toBe(right);
    });

    it('falls back to the default rather than vanishing on an unknown id', () => {
      const { container } = render(
        // Simulating a settings file from a future version, or a corrupt one.
        <CompanionFace state="idle" reducedMotion variety={'rupert' as never} />,
      );
      expect(eyes(container)).toHaveLength(2);
      expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);
    });

    it('runs no timers for any variety under reduced motion', () => {
      for (const id of FACE_VARIETY_IDS) {
        const { unmount } = render(
          <CompanionFace state="idle" reducedMotion variety={id} />,
        );
        expect(vi.getTimerCount()).toBe(0);
        act(() => {
          vi.advanceTimersByTime(60_000);
        });
        expect(vi.getTimerCount()).toBe(0);
        unmount();
      }
    });

    describe('shut', () => {
      const v = FACE_VARIETIES.shut;

      it('is already asleep when it mounts — waking up on launch is a greeting', () => {
        const { container } = render(
          <CompanionFace state="idle" reducedMotion={false} variety="shut" />,
        );
        expect(eyeHeights(container)).toEqual([v.shutHeight, v.shutHeight]);
        // And nothing whatsoever is scheduled: there is nothing to blink.
        expect(vi.getTimerCount()).toBe(0);
      });

      it('opens on the user’s key and goes back to sleep 400 ms after it is done', () => {
        const { container, rerender } = render(
          <CompanionFace state="idle" reducedMotion={false} variety="shut" />,
        );
        rerender(<CompanionFace state="listening" reducedMotion={false} variety="shut" />);
        expect(eyeHeights(container)).toEqual([EYE_HEIGHT.listening, EYE_HEIGHT.listening]);
        expect(eyeTransition(container)).toBe('all 200ms ease-out 0ms');

        rerender(<CompanionFace state="idle" reducedMotion={false} variety="shut" />);
        // Still awake: the close is late enough to read as settling rather
        // than as the tail of the resolve beat.
        act(() => {
          vi.advanceTimersByTime(v.sleepDelayMs - 1);
        });
        expect(eyeHeights(container)).toEqual([EYE_HEIGHT.idle, EYE_HEIGHT.idle]);

        act(() => {
          vi.advanceTimersByTime(1);
        });
        expect(eyeHeights(container)).toEqual([v.shutHeight, v.shutHeight]);
        expect(eyeTransition(container)).toBe(`all ${v.sleepMs}ms ease-in-out 0ms`);
      });

      it('closes instantly, with no timer, under reduced motion', () => {
        const { container, rerender } = render(
          <CompanionFace state="listening" reducedMotion variety="shut" />,
        );
        rerender(<CompanionFace state="idle" reducedMotion variety="shut" />);
        expect(eyeHeights(container)).toEqual([v.shutHeight, v.shutHeight]);
        expect(vi.getTimerCount()).toBe(0);
      });
    });

    it('makes `quick` narrow for 50 ms before it opens', () => {
      // The oldest trick in animation: anticipation is what makes fast motion
      // read as intentional rather than abrupt.
      const v = FACE_VARIETIES.quick;
      const { container } = render(
        <CompanionFace state="listening" reducedMotion={false} variety="quick" />,
      );
      expect(eyeHeights(container)).toEqual([
        v.height.listening - 0.6,
        v.height.listening - 0.6,
      ]);
      act(() => {
        vi.advanceTimersByTime(v.wakeAnticipationMs);
      });
      expect(eyeHeights(container)).toEqual([v.height.listening, v.height.listening]);
    });

    it('skips the anticipation under reduced motion — it is pure animation', () => {
      const v = FACE_VARIETIES.quick;
      const { container } = render(
        <CompanionFace state="listening" reducedMotion variety="quick" />,
      );
      expect(eyeHeights(container)).toEqual([v.height.listening, v.height.listening]);
      expect(vi.getTimerCount()).toBe(0);
    });

    it('opens `drowsy` more slowly than it closes it, and 80 ms late', () => {
      const v = FACE_VARIETIES.drowsy;
      const { container } = render(
        <CompanionFace state="listening" reducedMotion={false} variety="drowsy" />,
      );
      expect(eyeTransition(container)).toBe('all 260ms ease-out 80ms');
      expect(v.blink!.openMs).toBeGreaterThan(v.blink!.closeMs);
    });

    it('gives `drowsy` a resolve with no bounce at all', () => {
      const v = FACE_VARIETIES.drowsy;
      const { container, rerender } = render(
        <CompanionFace state="thinking" reducedMotion={false} variety="drowsy" />,
      );
      rerender(<CompanionFace state="resolving" reducedMotion={false} variety="drowsy" />);
      act(() => {
        vi.advanceTimersByTime(v.resolve.delayMs);
      });
      // It arrives at rest and does not overshoot on the way.
      expect(eyeHeights(container)).toEqual([v.height.idle, v.height.idle]);
      expect(eyeTransition(container)).toBe(`all ${v.resolve.riseMs}ms ease-out 0ms`);
    });

    it('runs each variety’s own blink rhythm, not house’s', () => {
      const randoms = [0.5, 0, 0.99];
      let i = 0;
      vi.spyOn(Math, 'random').mockImplementation(
        () => randoms[Math.min(i++, randoms.length - 1)],
      );
      const v = FACE_VARIETIES.quick;
      const { container } = render(
        <CompanionFace state="idle" reducedMotion={false} variety="quick" />,
      );
      // House would not have blinked for another second at this point.
      act(() => {
        vi.advanceTimersByTime(v.blink!.minMs);
      });
      expect(eyeHeights(container)).toEqual([v.shutHeight, v.shutHeight]);
      expect(eyeTransition(container)).toBe(`all ${v.blink!.closeMs}ms ease-in`);
      expect(v.blink!.minMs).toBeLessThan(BLINK_MIN_MS);
      vi.mocked(Math.random).mockRestore();
    });

    it('breathes on each variety’s own period, and not at all where there is none', () => {
      const { container, unmount } = render(
        <CompanionFace state="idle" reducedMotion={false} variety="drowsy" />,
      );
      expect(container.querySelector('.ttp-face-breath-drowsy')).not.toBeNull();
      expect(container.querySelector('style')!.textContent).toContain('6500ms');
      unmount();

      for (const id of ['shut', 'bead'] as const) {
        const still = render(<CompanionFace state="idle" reducedMotion={false} variety={id} />);
        expect(still.container.querySelector(`.ttp-face-breath-${id}`)).toBeNull();
        expect(still.container.querySelector('style')).toBeNull();
        still.unmount();
      }
    });
  });
});
