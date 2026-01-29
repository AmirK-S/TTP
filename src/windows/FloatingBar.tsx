// TTP - Talk To Paste
// Floating bar component - transparent recording indicator overlay

import { useEffect, useCallback, useState } from 'react';
import { useRecordingState } from '../hooks/useRecordingState';
import { useRecordingControl } from '../hooks/useRecordingControl';
import { useTranscription } from '../hooks/useTranscription';

/**
 * FloatingBar is a minimal, transparent overlay that appears when recording.
 * It displays a pulsing red indicator and "Recording..." text to provide
 * visual feedback to the user during voice capture.
 *
 * This component is rendered in its own transparent, always-on-top window.
 * It also hooks into the mic-recorder plugin to capture audio.
 */
export function FloatingBar() {
  const recordingState = useRecordingState();

  // Handle recording completion - log the file path for now
  // Phase 2 will use this for transcription
  const handleRecordingComplete = useCallback(
    (result: { filePath: string; duration?: number }) => {
      console.log(
        '[FloatingBar] Recording complete:',
        result.filePath,
        result.duration ? `(${result.duration.toFixed(1)}s)` : ''
      );
    },
    []
  );

  const handleRecordingError = useCallback((error: string) => {
    console.error('[FloatingBar] Recording error:', error);
  }, []);

  // Connect to mic-recorder plugin
  useRecordingControl({
    onRecordingComplete: handleRecordingComplete,
    onError: handleRecordingError,
  });

  // Set up transparent background for the window
  useEffect(() => {
    // Make html and body transparent for this window
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
  }, []);

  const { isProcessing } = useTranscription();
  const isRecording = recordingState === 'Recording';
  const [wavePhase, setWavePhase] = useState(0);
  const [processingPhase, setProcessingPhase] = useState(0);

  // Smooth flowing wave animation when recording
  useEffect(() => {
    if (!isRecording) return;

    let animationId: number;
    const animate = () => {
      setWavePhase(p => p + 0.12);
      animationId = requestAnimationFrame(animate);
    };
    animationId = requestAnimationFrame(animate);

    return () => cancelAnimationFrame(animationId);
  }, [isRecording]);

  // Pulsing animation when processing
  useEffect(() => {
    if (!isProcessing) return;

    let animationId: number;
    const animate = () => {
      setProcessingPhase(p => p + 0.05);
      animationId = requestAnimationFrame(animate);
    };
    animationId = requestAnimationFrame(animate);

    return () => cancelAnimationFrame(animationId);
  }, [isProcessing]);

  // Smooth flowing wave for recording
  const audioLevels = [0, 1, 2, 3, 4].map(i => {
    const wave = Math.sin(wavePhase + i * 0.6);
    return 0.5 + wave * 0.35;
  });

  // Pulsing opacity for processing indicator
  const processingOpacity = 0.6 + Math.sin(processingPhase) * 0.4;

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-transparent">
      <div
        className={`flex items-center justify-center rounded-full shadow-lg transition-all duration-150 ease-out ${
          isRecording
            ? 'gap-0.5 bg-black/90 px-2.5 py-1.5'
            : isProcessing
              ? 'bg-blue-500/80 px-3 py-1.5'
              : 'bg-gray-400/60 px-5 py-2'
        }`}
        style={{
          minHeight: isRecording ? '28px' : isProcessing ? '26px' : '16px',
          minWidth: isRecording ? '50px' : isProcessing ? '70px' : '40px',
          opacity: isProcessing ? processingOpacity : 1,
        }}
      >
        {isRecording && (
          <>
            {audioLevels.map((level, i) => (
              <span
                key={i}
                className="w-1 rounded-full bg-white/80 transition-all duration-75"
                style={{ height: `${level * 14}px` }}
              />
            ))}
          </>
        )}
        {isProcessing && (
          <span className="text-xs font-medium text-white">Processing...</span>
        )}
      </div>
    </div>
  );
}

export default FloatingBar;
