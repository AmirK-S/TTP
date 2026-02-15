// TTP - Talk To Paste
// Floating bar component - dark pill overlay with voice-reactive wave bars

import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useRecordingState } from '../hooks/useRecordingState';
import { useTranscription } from '../hooks/useTranscription';

const TUTORIAL_SHOWN_KEY = 'ttp_tutorial_shown';

// Wave bar configuration — Whisper-style flowing waveform
const BAR_COUNT = 12;
const MIN_HEIGHT = 2;
const MAX_HEIGHT = 20;

export function FloatingBar() {
  const recordingState = useRecordingState();

  useEffect(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
  }, []);

  const { stage, message, isProcessing: isTranscribing } = useTranscription();
  const isRecording = recordingState === 'Recording';
  // Treat both Rust "Processing" state AND transcription progress as processing
  // This eliminates the flash between recording end and first progress event
  const isProcessing = isTranscribing || recordingState === 'Processing';
  const isError = stage === 'error';
  const isIdle = !isRecording && !isProcessing;

  // First-launch tutorial
  const [showTutorial, setShowTutorial] = useState(() => {
    return !localStorage.getItem(TUTORIAL_SHOWN_KEY);
  });

  useEffect(() => {
    if (isRecording && showTutorial) {
      localStorage.setItem(TUTORIAL_SHOWN_KEY, '1');
      setShowTutorial(false);
    }
  }, [isRecording, showTutorial]);

  // Voice-reactive bars: driven by audio-level events from Rust
  const barRefs = useRef<(HTMLSpanElement | null)[]>([]);
  const levelRef = useRef(0);
  const rafRef = useRef(0);

  useEffect(() => {
    if (!isRecording) {
      cancelAnimationFrame(rafRef.current);
      // Reset bars to minimum height
      barRefs.current.forEach((bar) => {
        if (bar) bar.style.height = `${MIN_HEIGHT}px`;
      });
      return;
    }

    // Listen for audio level events from Rust audio monitor
    const unlistenPromise = listen<number>('audio-level', (event) => {
      levelRef.current = event.payload;
    });

    // Animation loop: update bar heights based on current audio level
    function animate() {
      const level = levelRef.current;
      const time = performance.now() / 1000;

      barRefs.current.forEach((bar, i) => {
        if (!bar) return;
        // Envelope: center bars taller, edges fade out (bell curve)
        const center = (BAR_COUNT - 1) / 2;
        const dist = Math.abs(i - center) / center; // 0 at center, 1 at edges
        const envelope = 1.0 - dist * dist * 0.6; // gentle falloff

        // Two traveling sine waves for organic flowing motion
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

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-end pb-1 bg-transparent pointer-events-none">

      {/* Tutorial popup — first launch only */}
      {showTutorial && isIdle && (
        <div className="tutorial-enter mb-2 flex items-center gap-2 rounded-2xl bg-black/90 px-4 py-2 shadow-xl backdrop-blur-sm">
          <span className="text-[13px] font-medium text-white/80 select-none">
            Maintiens{' '}
            <span className="inline-flex items-center justify-center rounded bg-white/20 px-1.5 py-0.5 text-[11px] font-bold text-white leading-none">
              fn
            </span>
            {' '}pour dicter
          </span>
        </div>
      )}

      {/* Pill */}
      <div
        className={`flex items-center justify-center rounded-full shadow-lg transition-all duration-150 ease-out ${
          isRecording
            ? 'gap-[1.5px] bg-black/90 px-3 py-1.5'
            : isError
              ? 'bg-red-500/90 px-3 py-1.5'
              : isProcessing
                ? 'pill-pulse bg-blue-500/80 px-3 py-1.5'
                : 'bg-gray-400/60 px-5 py-2'
        }`}
        style={{
          minHeight: isRecording ? '28px' : (isProcessing || isError) ? '26px' : '16px',
          minWidth: isRecording ? '56px' : isError ? '80px' : isProcessing ? '70px' : '40px',
        }}
      >
        {/* Recording: Whisper-style flowing waveform */}
        {isRecording && Array.from({ length: BAR_COUNT }).map((_, i) => (
          <span
            key={i}
            ref={(el) => { barRefs.current[i] = el; }}
            className="w-[2px] rounded-full bg-white/90"
            style={{ height: `${MIN_HEIGHT}px`, transition: 'height 50ms ease-out' }}
          />
        ))}

        {isError && (
          <span className="text-xs font-medium text-white whitespace-nowrap">{message || 'Error'}</span>
        )}

        {isProcessing && !isError && (
          <span className="text-xs font-medium text-white whitespace-nowrap">Transcription...</span>
        )}
      </div>
    </div>
  );
}

export default FloatingBar;
