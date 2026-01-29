// TTP - Talk To Paste
// Floating bar component - transparent recording indicator overlay

import { useEffect, useCallback } from 'react';
import { Mic } from 'lucide-react';
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

  // Only show content when recording
  // (though the window visibility is controlled by Rust)
  const isRecording = recordingState === 'Recording';

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-transparent">
      {isRecording && (
        <div className="flex items-center gap-2 rounded-full bg-black/80 px-4 py-2 text-white backdrop-blur-sm shadow-lg">
          {/* Pulsing red recording indicator */}
          <span className="h-3 w-3 animate-pulse rounded-full bg-red-500" />

          {/* Microphone icon */}
          <Mic className="h-4 w-4" />

          {/* Status text */}
          <span className="text-sm font-medium">Recording...</span>
        </div>
      )}
    </div>
  );
}

export default FloatingBar;
