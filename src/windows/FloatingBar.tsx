// TTP - Talk To Paste
// Floating bar component - dark pill overlay with voice-reactive waveform,
// elapsed timer and discrete state-driven appearance (idle / recording /
// transcribing / completing / error). Lives in the transparent floating-bar
// window.
//
// The completion frame is new, and it is the point of workstream Z3. Until
// Polaris the pill had *no* completion state at all: `stage === 'complete'`
// still satisfied `isProcessing`, so a finished dictation went on saying
// "Transcribing…" for another 500 ms and then vanished. Four different
// terminal facts — observed, posted-but-unseen, observed *not* to have
// landed, and never injected — rendered as that one identical frame, which is
// the user-facing half of the defect `src-tauri/src/paste/outcome.rs` fixed in
// the trace. See `src/lib/pasteOutcome.ts` for what each one now draws and
// why, and `src/hooks/usePasteCompletion.ts` for when it is allowed to draw
// it.

import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { AlertCircle, Check, CheckCheck } from 'lucide-react';
import { safeInvoke } from '../lib/safeInvoke';
import { translateRustMessage } from '../lib/translateRustMessage';
import { useRecordingState } from '../hooks/useRecordingState';
import { useRecordingMode } from '../hooks/useRecordingMode';
import { useTranscription } from '../hooks/useTranscription';
import { usePasteCompletion } from '../hooks/usePasteCompletion';
import { TutorialPill } from '../components/TutorialPill';
import { DarkPill } from '../components/ui';
import { CompanionFace } from '../components/ui/CompanionFace';
import {
  DEFAULT_FACE_VARIETY,
  asFaceVariety,
  type FaceState,
  type FaceVarietyId,
} from '../components/ui/companion-timing';
import { treatmentFor, type OutcomeMark } from '../lib/pasteOutcome';
import { cn } from '../lib/cn';
import { Lock } from 'lucide-react';

const TUTORIAL_DISMISSED_KEY = 'tutorial_pill_dismissed';

const BAR_COUNT = 14;
const MIN_HEIGHT = 2;
const MAX_HEIGHT = 16;

/**
 * Subscribe to `prefers-reduced-motion` rather than reading it once.
 *
 * The shipped code read `.matches` at render with no `change` listener, so a
 * user who turned Reduce Motion on while TTP was running kept the old
 * behaviour until the window reloaded — and the floating bar is a window that
 * essentially never reloads.
 */
export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return false;
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  });

  useEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return;
    const query = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onChange = (event: MediaQueryListEvent) => setReduced(event.matches);
    setReduced(query.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  }, []);

  return reduced;
}

function formatElapsed(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const mm = Math.floor(totalSec / 60);
  const ss = totalSec % 60;
  return `${mm}:${ss.toString().padStart(2, '0')}`;
}

export function FloatingBar() {
  const { t } = useTranslation();
  const recordingState = useRecordingState();

  /* The Companion's face, and which variety of it. Off unless the user both
     owns and chose one — asked once at mount and refreshed when settings
     change, because this window has no settings store of its own.

     Two shapes of setting are accepted on purpose. `companion_face` is a
     variety id (or null for off) and is what the picker writes; the older
     `companion_face_enabled` boolean is only consulted when the newer field is
     absent entirely, so an install that predates the cast shows `house` rather
     than nothing, and an install that has the field never has a stale boolean
     resurrect a face the user turned off. */
  const [face, setFace] = useState<FaceVarietyId | null>(null);
  useEffect(() => {
    const refresh = async () => {
      const [settings, unlocked] = await Promise.all([
        safeInvoke<{ companion_face_enabled?: boolean; companion_face?: string | null }>(
          'get_settings',
        ),
        safeInvoke<boolean>('cosmetics_unlocked'),
      ]);
      const chosen =
        settings && 'companion_face' in settings
          ? asFaceVariety(settings.companion_face)
          : settings?.companion_face_enabled
            ? DEFAULT_FACE_VARIETY
            : null;
      setFace(unlocked ? chosen : null);
    };
    refresh();
    const un = listen('settings-changed', refresh);
    return () => { un.then((f) => f()); };
  }, []);

  useEffect(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
    /* The coat layer resolves `--ttp-surface` and `--ttp-ink` differently in
       this window — here the surface IS the pill and the ink is the mark that
       has to stay legible on it, over whatever wallpaper is behind. That block
       is keyed on `html.floating-bar`, so the class has to be on. */
    document.documentElement.classList.add('floating-bar');
  }, []);

  const { stage, message, params, isProcessing: isTranscribing } = useTranscription();
  const translatedMessage = translateRustMessage(t, message, params);
  const recordingMode = useRecordingMode();
  const isHandsFree = recordingMode === 'toggle';
  const isRecording = recordingState === 'Recording';
  // Treat both Rust "Processing" state AND transcription progress as processing
  // — eliminates the flicker between recording end and first progress event.
  const isProcessing = isTranscribing || recordingState === 'Processing';
  const isError = stage === 'error';

  /* Which of the four paste outcomes to report, or null when this build of
     Rust did not say — in which case nothing is drawn and the pill behaves
     exactly as it did before. The frame outlives `stage`: `useTranscription`
     returns to `idle` 500 ms after `complete`, and the unverified frame runs
     to 1500 ms because it has words in it that have to be readable. */
  const settledOutcome = usePasteCompletion(stage, params);
  /* The next recording always wins. `recording-state-changed` arrives on the
     key press, before any progress event the hook could react to, so without
     this a fast second dictation would draw its waveform and the previous
     dictation's report in the same pill. */
  const completion = isRecording ? null : settledOutcome;
  const treatment = completion ? treatmentFor(completion) : null;
  const isCompleting = treatment !== null;
  const isDanger = isError || treatment?.tone === 'danger';

  const isIdle = !isRecording && !isProcessing && !isError && !isCompleting;
  /* Everything that makes the pill grow to 28px and take its working padding.
     The completion frame is one of them, so a finished dictation shrinks once,
     when it is finished, rather than shrinking and then having to grow again. */
  const isWorking = isRecording || isProcessing || isError || isCompleting;

  /* Dead microphone, announced while there is still time to act on it. -------
     Emitted by audio_monitor once a capture has passed its grace period with
     nothing but silence. Cleared whenever recording stops, so the warning is
     about this recording and never a leftover from the last one. */
  const [deadInput, setDeadInput] = useState(false);
  useEffect(() => {
    if (!isRecording) setDeadInput(false);
  }, [isRecording]);
  useEffect(() => {
    const un = listen('audio-dead-input', () => setDeadInput(true));
    return () => { un.then((f) => f()); };
  }, []);

  /* Elapsed timer during recording. Reset on each recording start. ----------- */
  const [elapsedMs, setElapsedMs] = useState(0);
  useEffect(() => {
    if (!isRecording) {
      setElapsedMs(0);
      return;
    }
    const start = performance.now();
    const id = window.setInterval(() => setElapsedMs(performance.now() - start), 100);
    return () => window.clearInterval(id);
  }, [isRecording]);

  /* First-launch tutorial pill. ---------------------------------------------- */
  const [showTutorial, setShowTutorial] = useState(false);
  useEffect(() => {
    const checkFirstLaunch = async () => {
      try {
        const isFirst = await safeInvoke<boolean>('is_first_launch_cmd');
        const dismissed = localStorage.getItem(TUTORIAL_DISMISSED_KEY) === 'true';
        setShowTutorial(isFirst && !dismissed);
      } catch {
        const dismissed = localStorage.getItem(TUTORIAL_DISMISSED_KEY) === 'true';
        setShowTutorial(!dismissed);
      }
    };
    checkFirstLaunch();
  }, []);
  useEffect(() => {
    if (stage === 'complete' && showTutorial) {
      localStorage.setItem(TUTORIAL_DISMISSED_KEY, 'true');
      setShowTutorial(false);
    }
  }, [stage, showTutorial]);

  /* Voice-reactive waveform, driven by audio-level events from Rust. --------
   *
   * Respect prefers-reduced-motion: vestibular sensitivity gets aggravated by
   * the continuous 60fps bar oscillation. When the user opts into reduced
   * motion, we mount the bars at a clean static silhouette and skip the RAF
   * loop entirely — recording state is still conveyed by the red dot, the
   * timer, and aria-live on the pill, so there's no information loss. */
  const prefersReducedMotion = usePrefersReducedMotion();
  const barRefs = useRef<(HTMLSpanElement | null)[]>([]);
  const levelRef = useRef(0);
  const rafRef = useRef(0);

  useEffect(() => {
    if (!isRecording) {
      cancelAnimationFrame(rafRef.current);
      // Collapse to minimum via transform (no layout pass).
      barRefs.current.forEach((bar) => {
        if (bar) bar.style.transform = `scaleY(${MIN_HEIGHT / MAX_HEIGHT})`;
      });
      return;
    }

    if (prefersReducedMotion) {
      // Render a static, mid-height waveform silhouette and skip RAF. The
      // user still sees "recording" via the timer + tone + aria-live; what
      // we drop is the continuous motion that vestibular users find painful.
      barRefs.current.forEach((bar, i) => {
        if (!bar) return;
        const center = (BAR_COUNT - 1) / 2;
        const dist = Math.abs(i - center) / center;
        const envelope = 1.0 - dist * dist * 0.55;
        bar.style.transform = `scaleY(${envelope * 0.5 + 0.25})`;
      });
      return;
    }

    const unlistenPromise = listen<number>('audio-level', (event) => {
      levelRef.current = event.payload;
    });

    function animate() {
      const level = levelRef.current;
      const time = performance.now() / 1000;
      barRefs.current.forEach((bar, i) => {
        if (!bar) return;
        // Bell-curve envelope so the centre bars sit taller than the edges,
        // giving the pill a clean rounded waveform silhouette.
        const center = (BAR_COUNT - 1) / 2;
        const dist = Math.abs(i - center) / center;
        const envelope = 1.0 - dist * dist * 0.55;
        // Two phased sines for an organic flowing motion.
        const wave1 = Math.sin(time * 3.2 + i * 0.45);
        const wave2 = Math.sin(time * 2.1 + i * 0.7 + 1.2);
        const wave = Math.abs(wave1 * 0.6 + wave2 * 0.4);
        const h = MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * level * envelope * wave;
        // GPU-composited transform — no layout/paint, only the compositor runs.
        // transform accepts raw numbers; skip the toFixed allocation per frame.
        bar.style.transform = `scaleY(${h / MAX_HEIGHT})`;
      });
      rafRef.current = requestAnimationFrame(animate);
    }
    rafRef.current = requestAnimationFrame(animate);
    return () => {
      cancelAnimationFrame(rafRef.current);
      unlistenPromise.then((fn) => fn());
    };
  }, [isRecording, prefersReducedMotion]);

  /* The face's state.
     `resolving` is the beat when the words land — 140 ms late, because it is a
     reaction to the app's own result rather than to the user's key, and that
     delay is the whole difference between a status light and a thing that
     noticed. `stage === 'complete'` holds for 500 ms (useTranscription), which
     comfortably contains the 400 ms beat. */
  /* The resolve beat is a claim, so it is now spent only where there is
     evidence for it.

     It is the face's 140 ms-late overshoot — "the thing that noticed" — and it
     used to fire on `stage === 'complete'`, which is to say on all four
     outcomes including the swallowed ones. That is the concealment expressed
     in animation: the Companion nodding at a paste nobody watched arrive.
     `pasted` keeps the nod. `pasted_unverified` simply does not get one: the
     face returns to rest, having done the work and not seen the result. A
     withheld gesture costs no pixels, no milliseconds and no attention, and a
     person who dictates fifty times a day will feel its absence.

     A swallow wears the error face, because that is what it is. */
  const faceState: FaceState = isError || completion === 'paste_swallowed'
    ? 'error'
    : completion === 'pasted'
      ? 'resolving'
      : completion !== null
        ? 'idle'
        : stage === 'complete'
          ? 'resolving' // no outcome on the wire: unchanged behaviour
          : isProcessing
            ? 'thinking'
            : isRecording
              ? 'listening'
              : 'idle';

  /* At rest with a face on there is nothing else in the pill, so the pill can
     stop being a bar with a face in it and become the face's body. */
  const faceIsThePill = face !== null && isIdle;

  const pillTone: 'idle' | 'active' | 'danger' = isDanger
    ? 'danger'
    : isRecording || isProcessing || isCompleting
      ? 'active'
      : 'idle';

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-end pb-1 bg-transparent pointer-events-none">
      {showTutorial && isIdle && <TutorialPill shortcutText="FN" />}

      <DarkPill
        tone={pillTone}
        className={cn(
          'flex items-center gap-2 transition-[background-color,color,padding] duration-200 ease-app-out',
          isWorking
            ? 'px-3.5 py-1.5'
            : /* At rest with a face on, the face IS the pill: the padding comes
                 off so the 16px box sets the height exactly, and the sides pull
                 in so the body hugs it. Rendered at 1x and looked at — with the
                 old `py-1.5` the idle pill stood 28px tall for two 3px marks
                 floating in the middle of it, which is what made the face read
                 as a decal on a bar rather than as a head. */
              faceIsThePill
              ? 'px-[7px] py-0'
              : 'px-4 py-1.5',
          /* The tremble. `docs/companion-manual.md`: "This is the entire
             emotional range of the animal." It is spent on errors and on a
             swallowed paste, and on nothing else — an unverified paste is not
             an error and must never shake. Clamped to 0.01ms by the
             `prefers-reduced-motion` block in `src/index.css`. */
          isDanger && 'anim-shake',
        )}
        /* The body's colour is `DarkPill`'s business and it already reads the
           coat. Nothing here overrides it — a second declaration in this file
           would win on the style spread and silently undo it. */
        style={{ minHeight: isWorking ? 28 : 16 }}
        role="status"
        aria-live="polite"
      >
        {face && (
          <CompanionFace
            state={faceState}
            reducedMotion={prefersReducedMotion}
            variety={face}
          />
        )}

        {isRecording && (
          <>
            <span className="flex items-end gap-[2px] h-[16px]" aria-hidden>
              {Array.from({ length: BAR_COUNT }).map((_, i) => (
                <span
                  key={i}
                  ref={(el) => { barRefs.current[i] = el; }}
                  className="w-[2px] rounded-full bg-white/90"
                  style={{
                    height: `${MAX_HEIGHT}px`,
                    // Anchor the scale at the bottom so the bar grows up, not from center.
                    transformOrigin: 'bottom',
                    // Start collapsed; the RAF loop overrides this once recording begins.
                    transform: `scaleY(${MIN_HEIGHT / MAX_HEIGHT})`,
                    // Hint the compositor — keeps the layer hot for smoother updates.
                    willChange: 'transform',
                  }}
                />
              ))}
            </span>
            {deadInput ? (
              // Replaces the timer rather than sitting beside it: a counter
              // ticking up next to "no sound" reads as though the recording is
              // fine, which is the impression we are trying to correct.
              <span className="text-[11px] font-medium text-app-warning whitespace-nowrap">
                {t('floatingBar.deadInput')}
              </span>
            ) : (
              <span className="text-[11px] font-medium tabular-nums text-white/90 tracking-tight">
                {formatElapsed(elapsedMs)}
              </span>
            )}
            {isHandsFree && (
              <Lock
                className="size-3 text-white/80 shrink-0"
                aria-label={t('floatingBar.handsFreeIndicatorLabel')}
              />
            )}
          </>
        )}

        {isProcessing && !isError && !isCompleting && (
          <>
            <span
              className="block size-2 rounded-full bg-white/90 anim-pulse"
              aria-hidden
            />
            <span className="text-xs font-medium text-white/95 whitespace-nowrap">
              {t('floatingBar.statusTranscribing')}
            </span>
          </>
        )}

        {treatment && !isError && (
          <>
            <CompletionMark mark={treatment.mark} reducedMotion={prefersReducedMotion} />
            {treatment.line && (
              <span
                className={cn(
                  'text-xs font-medium whitespace-nowrap',
                  /* 70% on the unverified line, full white on the danger one.
                     A caption and a warning are different volumes, and the
                     state a user meets several times a day on a dictation
                     where nothing went wrong gets the caption. */
                  treatment.tone === 'danger' ? 'text-white' : 'text-white/70',
                )}
              >
                {t(treatment.line)}
              </span>
            )}
            {/* The asymmetry, stated on purpose. A sighted user can look at
                their own text field, which is a better verifier than any
                Accessibility read, so the visual channel gets a mark and at
                most four words. A screen-reader user cannot look, so the live
                region gets the whole sentence — for every outcome, including
                the one that draws nothing. */}
            <span className="sr-only">{t(treatment.announcement)}</span>
          </>
        )}

        {isError && (
          <>
            <AlertCircle className="size-3.5 text-white" aria-hidden />
            <span className="text-xs font-medium text-white whitespace-nowrap">
              {translatedMessage || t('floatingBar.errorFallback')}
            </span>
          </>
        )}
      </DarkPill>
    </div>
  );
}

/**
 * The mark the completion frame draws, and the only place the four outcomes
 * differ visually for a user who has no Companion face turned on — which is
 * most users, since the face is off by default. Whatever the honest treatment
 * is, it cannot live in the face alone.
 *
 * `arrived` pops and `sent` fades, which is the same distinction the face
 * makes with its resolve beat, restated for the faceless pill: a paste we
 * watched arrive is acknowledged, a paste we merely posted is not. Both
 * animations are single-shot CSS on a 14px glyph — no RAF, nothing that
 * outlives the frame — and `src/index.css`'s `prefers-reduced-motion` block
 * clamps them to 0.01 ms. The `reducedMotion` prop drops the class outright
 * rather than relying on that; the states survive, only the motion goes,
 * which is the rule `CompanionFace` already follows.
 */
function CompletionMark({
  mark,
  reducedMotion,
}: {
  mark: OutcomeMark;
  reducedMotion: boolean;
}) {
  if (mark === 'alert') {
    return (
      <AlertCircle
        className={cn('size-3.5 shrink-0 text-white', !reducedMotion && 'anim-fade-in')}
        aria-hidden
      />
    );
  }
  if (mark === 'arrived') {
    return (
      <CheckCheck
        className={cn('size-3.5 shrink-0 text-white', !reducedMotion && 'anim-check-pop')}
        aria-hidden
        data-ttp-mark="arrived"
      />
    );
  }
  /* One tick, at the caption's weight. It is the same ink as the words beside
     it because it means the same thing they do, and because a full-strength
     mark next to a 70% line reads as an alert badge with an excuse attached. */
  return (
    <Check
      className={cn('size-3.5 shrink-0 text-white/70', !reducedMotion && 'anim-fade-in')}
      aria-hidden
      data-ttp-mark="sent"
    />
  );
}

export default FloatingBar;
