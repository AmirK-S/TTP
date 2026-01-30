// TTP - Talk To Paste
// Hook to control actual microphone recording via tauri-plugin-mic-recorder
// This hooks into the recording-state-changed events from Rust and
// starts/stops the mic recording plugin accordingly.

import { startRecording, stopRecording } from 'tauri-plugin-mic-recorder-api';
import { invoke } from '@tauri-apps/api/core';
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

// Debug helper to log to terminal
const debugLog = (message: string) => {
  invoke('debug_log', { message }).catch(() => {});
};

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
    debugLog('handleStartRecording called, isRecording=' + isRecordingRef.current);
    if (isRecordingRef.current) {
      debugLog('Already recording, skipping');
      return; // Already recording
    }

    try {
      isRecordingRef.current = true;
      recordingStartTime.current = Date.now();
      debugLog('Calling startRecording()...');
      await startRecording();
      debugLog('startRecording() succeeded');
    } catch (error) {
      debugLog('startRecording() FAILED: ' + String(error));
      isRecordingRef.current = false;
      recordingStartTime.current = null;
      onError?.(String(error));
    }
  }, [onError]);

  const handleStopRecording = useCallback(async () => {
    debugLog('handleStopRecording called, isRecording=' + isRecordingRef.current);
    if (!isRecordingRef.current) {
      debugLog('Not recording, skipping stop');
      return; // Not recording
    }

    try {
      isRecordingRef.current = false;
      const duration = recordingStartTime.current
        ? (Date.now() - recordingStartTime.current) / 1000
        : 0;
      recordingStartTime.current = null;
      debugLog('Recording duration: ' + duration.toFixed(2) + 's');

      // Skip very short recordings (< 0.3s) - likely accidental
      if (duration < 0.3) {
        debugLog('Too short (<0.3s), skipping and resetting');
        try {
          await stopRecording(); // Still need to stop the recorder
        } catch {
          // Ignore stop errors for short recordings
        }
        // Reset state to Idle so user can record again
        await invoke('reset_to_idle');
        return;
      }

      debugLog('Calling stopRecording()...');
      const filePath = await stopRecording();
      debugLog('stopRecording() returned: ' + filePath);

      onRecordingComplete?.({
        filePath,
        duration,
      });

      // Trigger transcription pipeline
      debugLog('Starting transcription pipeline with: ' + filePath);
      invoke('process_audio', { audioPath: filePath })
        .then((result) => {
          debugLog('Transcription complete: ' + String(result));
        })
        .catch((error) => {
          debugLog('Transcription FAILED: ' + String(error));
          onError?.(String(error));
        });
    } catch (error) {
      debugLog('handleStopRecording FAILED: ' + String(error));
      recordingStartTime.current = null;
      onError?.(String(error));
    }
  }, [onRecordingComplete, onError]);

  useEffect(() => {
    debugLog('useRecordingControl: Setting up event listener');
    // Listen for recording state changes from Rust
    const unlisten = listen<RecordingState>('recording-state-changed', async (event) => {
      const state = event.payload;
      debugLog('Event received: ' + state + ', isRecording=' + isRecordingRef.current);

      if (state === 'Recording' && !isRecordingRef.current) {
        await handleStartRecording();
      } else if (state === 'Processing' && isRecordingRef.current) {
        // Recording stopped, now processing - stop mic and trigger pipeline
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
