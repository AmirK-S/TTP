// TTP - Talk To Paste
// Tests for the Companion face as rendered.
//
// The timing logic is covered by companion-timing.test.ts against a virtual
// clock. What this file checks is that the component actually *uses* it: that
// the eyes are not shut during transcription, that the beat runs when the
// words land, that reduced motion removes loops and transitions while keeping
// states, and that there is no mouth.

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
} from './companion-timing';

/** Heights of the two eye rects, as the DOM actually has them. */
function eyeHeights(container: HTMLElement): number[] {
  return Array.from(container.querySelectorAll('rect')).map((r) =>
    Number(r.getAttribute('height')),
  );
}

function eyeTransition(container: HTMLElement): string {
  return container.querySelector('rect')!.style.transition;
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
      expect(container.querySelectorAll('path')).toHaveLength(0);
      expect(container.querySelectorAll('rect')).toHaveLength(2);
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
    expect(container.querySelector('rect')!.style.fillOpacity).toBe('0.55');
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
      expect(container.querySelector('.ttp-face-breath')).toBeNull();
    });

    it('runs the breath at idle when motion is allowed, and only at idle', () => {
      const { container, rerender } = render(
        <CompanionFace state="idle" reducedMotion={false} />,
      );
      const breath = container.querySelector('.ttp-face-breath') as HTMLElement | null;
      expect(breath).not.toBeNull();
      // A negative delay, so the phase is not in lockstep with the clock, the
      // caret, or anything else on screen.
      expect(Number(breath!.style.animationDelay.replace('ms', ''))).toBeLessThanOrEqual(0);

      rerender(<CompanionFace state="thinking" reducedMotion={false} />);
      expect(container.querySelector('.ttp-face-breath')).toBeNull();
    });

    it('breathes on an HTML element, never on SVG innards', () => {
      // A transform keyframe on an SVG <g> is not reliably composited in
      // WebKit, and this animation runs forever on battery. It has to be on
      // something the compositor will promote.
      const { container } = render(<CompanionFace state="idle" reducedMotion={false} />);
      const breath = container.querySelector('.ttp-face-breath')!;
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
});
