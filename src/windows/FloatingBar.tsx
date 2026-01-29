// TTP - Talk To Paste
// Floating bar component - transparent recording indicator overlay

import { useEffect } from 'react';
import { Mic } from 'lucide-react';
import { useRecordingState } from '../hooks/useRecordingState';

/**
 * FloatingBar is a minimal, transparent overlay that appears when recording.
 * It displays a pulsing red indicator and "Recording..." text to provide
 * visual feedback to the user during voice capture.
 *
 * This component is rendered in its own transparent, always-on-top window.
 */
export function FloatingBar() {
  const recordingState = useRecordingState();

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
