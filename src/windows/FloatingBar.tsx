// TTP - Talk To Paste
// Floating bar component - dark pill overlay with voice-reactive waveform,
// elapsed timer and discrete state-driven appearance (idle / recording /
// transcribing / error). Lives in the transparent floating-bar window.

import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { AlertCircle } from 'lucide-react';
import { safeInvoke } from '../lib/safeInvoke';
import { useRecordingState } from '../hooks/useRecordingState';
import { useTranscription } from '../hooks/useTranscription';
import { TutorialPill } from '../components/TutorialPill';

/**
 * Rust pipeline emits translation keys (e.g. `error.no_speech`,
 * `progress.transcribing`) in the `message` field — resolve them via i18n,
 * but fall through gracefully on any unforeseen literal string.
 */
function translateRustMessage(
  t: (key: string, params?: Record<string, string | number>) => string,
  message: string,
  params?: Record<string, string | number>,
): string {
  if (!message) return '';
  if (
    message.includes('.') &&
    (message.startsWith('error.') || message.startsWith('progress.') || message.startsWith('permission.'))
  ) {
    return t(message, params);
  }
  return message;
}

const TUTORIAL_DISMISSED_KEY = 'tutorial_pill_dismissed';
const BAR_COUNT = 14;
const MIN_HEIGHT = 2;
const MAX_HEIGHT = 22;

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

  /* Voice-reactive waveform, driven by audio-level events from Rust. -------- */
  const barRefs = useRef<(HTMLSpanElement | null)[]>([]);
  const levelRef = useRef(0);
  const rafRef = useRef(0);

  useEffect(() => {
    if (!isRecording) {
      cancelAnimationFrame(rafRef.current);
      barRefs.current.forEach((bar) => { if (bar) bar.style.height = `${MIN_HEIGHT}px`; });
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
        bar.style.height = `${h.toFixed(1)}px`;
      });
      rafRef.current = requestAnimationFrame(animate);
    }
    rafRef.current = requestAnimationFrame(animate);
    return () => {
      cancelAnimationFrame(rafRef.current);
      unlistenPromise.then((fn) => fn());
    };
  }, [isRecording]);

  /* Pill state styling. -------------------------------------------------------
   * Single source of truth for the per-state pill chrome — keeps the JSX below
   * tight and makes future state additions a one-line change. */
  const pillStateClass = isError
    ? 'bg-[#dc2626]/95 ring-1 ring-white/10'
    : isProcessing
      ? 'bg-black/90 ring-1 ring-white/10'
      : isRecording
        ? 'bg-black/90 ring-1 ring-white/10'
        : 'bg-black/55 ring-1 ring-white/5';

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-end pb-1 bg-transparent pointer-events-none">
      {showTutorial && isIdle && <TutorialPill shortcutText="FN" />}

      <div
        className={`
          flex items-center gap-2 rounded-full shadow-lg backdrop-blur-md
          transition-all duration-200 ease-out
          ${pillStateClass}
          ${isRecording ? 'px-3.5 py-1.5' : isProcessing || isError ? 'px-3.5 py-1.5' : 'px-4 py-1.5'}
        `}
        style={{
          minHeight: isRecording || isProcessing || isError ? 28 : 16,
        }}
        role="status"
        aria-live="polite"
      >
        {isRecording && (
          <>
            <span className="flex items-end gap-[2px]" aria-hidden>
              {Array.from({ length: BAR_COUNT }).map((_, i) => (
                <span
                  key={i}
                  ref={(el) => { barRefs.current[i] = el; }}
                  className="w-[2px] rounded-full bg-white/90"
                  style={{ height: `${MIN_HEIGHT}px`, transition: 'height 50ms ease-out' }}
                />
              ))}
            </span>
            <span className="text-[11px] font-medium tabular-nums text-white/90 tracking-tight">
              {formatElapsed(elapsedMs)}
            </span>
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
      </div>
    </div>
  );
}

export default FloatingBar;
