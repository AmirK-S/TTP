// TTP - Talk To Paste
// Hook to control actual microphone recording via tauri-plugin-mic-recorder
// This hooks into the recording-state-changed events from Rust and
// starts/stops the mic recording plugin accordingly.

import { startRecording, stopRecording } from 'tauri-plugin-mic-recorder-api';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useRef, useCallback } from 'react';

type RecordingState = 'Idle' | 'Recording' | 'Processing';

export interface RecordingResult {
  filePath: string;
  duration?: number;
}

interface UseRecordingControlOptions {
  onRecordingComplete?: (result: RecordingResult) => void;
  onError?: (error: string) => void;
}

/**
 * Hook that connects Rust recording state events to the mic-recorder plugin.
 * When Rust emits 'Recording' state (from shortcut press), we start mic recording.
 * When Rust emits 'Idle' state (from shortcut release), we stop and get the file path.
 */
export function useRecordingControl(options: UseRecordingControlOptions = {}) {
  const { onRecordingComplete, onError } = options;
  const isRecordingRef = useRef(false);
  const recordingStartTime = useRef<number | null>(null);

  const handleStartRecording = useCallback(async () => {
    if (isRecordingRef.current) {
      return; // Already recording
    }

    try {
      isRecordingRef.current = true;
      recordingStartTime.current = Date.now();
      await startRecording();
      console.log('[Recording] Microphone recording started');
    } catch (error) {
      console.error('[Recording] Failed to start recording:', error);
      isRecordingRef.current = false;
      recordingStartTime.current = null;
      onError?.(String(error));
    }
  }, [onError]);

  const handleStopRecording = useCallback(async () => {
    if (!isRecordingRef.current) {
      return; // Not recording
    }

    try {
      isRecordingRef.current = false;
      const filePath = await stopRecording();
      const duration = recordingStartTime.current
        ? (Date.now() - recordingStartTime.current) / 1000
        : undefined;
      recordingStartTime.current = null;

      console.log('[Recording] Saved to:', filePath, 'Duration:', duration?.toFixed(1), 's');

      onRecordingComplete?.({
        filePath,
        duration,
      });
    } catch (error) {
      console.error('[Recording] Failed to stop recording:', error);
      recordingStartTime.current = null;
      onError?.(String(error));
    }
  }, [onRecordingComplete, onError]);

  useEffect(() => {
    // Listen for recording state changes from Rust
    const unlisten = listen<RecordingState>('recording-state-changed', async (event) => {
      const state = event.payload;

      if (state === 'Recording' && !isRecordingRef.current) {
        await handleStartRecording();
      } else if (state === 'Idle' && isRecordingRef.current) {
        await handleStopRecording();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [handleStartRecording, handleStopRecording]);

  return {
    isRecording: isRecordingRef.current,
  };
}
