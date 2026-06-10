// TTP - Talk To Paste
// Floating bar component - dark pill overlay with voice-reactive waveform,
// elapsed timer and discrete state-driven appearance (idle / recording /
// transcribing / error). Lives in the transparent floating-bar window.

import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { AlertCircle } from 'lucide-react';
import { safeInvoke } from '../lib/safeInvoke';
import { translateRustMessage } from '../lib/translateRustMessage';
import { useRecordingState } from '../hooks/useRecordingState';
import { useRecordingMode } from '../hooks/useRecordingMode';
import { useTranscription } from '../hooks/useTranscription';
import { TutorialPill } from '../components/TutorialPill';
import { DarkPill } from '../components/ui';
import { cn } from '../lib/cn';
import { Lock } from 'lucide-react';

const TUTORIAL_DISMISSED_KEY = 'tutorial_pill_dismissed';
const BAR_COUNT = 14;
const MIN_HEIGHT = 2;
const MAX_HEIGHT = 16;

function formatElapsed(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const mm = Math.floor(totalSec / 60);
  const ss = totalSec % 60;
  return `${mm}:${ss.toString().padStart(2, '0')}`;
}

export function FloatingBar() {
  const { t } = useTranslation();
  const recordingState = useRecordingState();

  useEffect(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
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
  const isIdle = !isRecording && !isProcessing && !isError;

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
  const prefersReducedMotion =
    typeof window !== 'undefined' &&
    window.matchMedia &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;
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

  const pillTone: 'idle' | 'active' | 'danger' = isError
    ? 'danger'
    : isRecording || isProcessing
      ? 'active'
      : 'idle';

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-end pb-1 bg-transparent pointer-events-none">
      {showTutorial && isIdle && <TutorialPill shortcutText="FN" />}

      <DarkPill
        tone={pillTone}
        className={cn(
          'flex items-center gap-2 transition-[background-color,color,padding] duration-200 ease-app-out',
          isRecording || isProcessing || isError ? 'px-3.5 py-1.5' : 'px-4 py-1.5',
          isError && 'anim-shake',
        )}
        style={{
          minHeight: isRecording || isProcessing || isError ? 28 : 16,
        }}
        role="status"
        aria-live="polite"
      >
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
            <span className="text-[11px] font-medium tabular-nums text-white/90 tracking-tight">
              {formatElapsed(elapsedMs)}
            </span>
            {isHandsFree && (
              <Lock
                className="size-3 text-white/80 shrink-0"
                aria-label={t('floatingBar.handsFreeIndicatorLabel')}
              />
            )}
          </>
        )}

        {isProcessing && !isError && (
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

export default FloatingBar;
