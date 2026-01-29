// TTP - Talk To Paste
// Floating bar component - transparent recording indicator overlay

import { useEffect, useCallback } from 'react';
import { useRecordingState } from '../hooks/useRecordingState';
import { useRecordingControl } from '../hooks/useRecordingControl';

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

  const isRecording = recordingState === 'Recording';

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-transparent">
      {isRecording ? (
        // Recording state: black pill with pulsing dots
        <div className="flex items-center justify-center gap-1 rounded-full bg-black/90 px-3 py-1.5 shadow-lg">
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-white/80" style={{ animationDelay: '0ms' }} />
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-white/80" style={{ animationDelay: '150ms' }} />
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-white/80" style={{ animationDelay: '300ms' }} />
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-white/80" style={{ animationDelay: '450ms' }} />
          <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-white/80" style={{ animationDelay: '600ms' }} />
        </div>
      ) : (
        // Idle state: small subtle grey pill
        <div className="h-5 w-12 rounded-full bg-gray-400/60 shadow-sm" />
      )}
    </div>
  );
}

export default FloatingBar;
