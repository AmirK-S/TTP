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
      const duration = recordingStartTime.current
        ? (Date.now() - recordingStartTime.current) / 1000
        : 0;
      recordingStartTime.current = null;

      // Skip very short recordings (< 0.3s) - likely accidental
      if (duration < 0.3) {
        console.log('[Recording] Too short, skipping:', duration.toFixed(2), 's');
        try {
          await stopRecording(); // Still need to stop the recorder
        } catch {
          // Ignore stop errors for short recordings
        }
        // Reset state to Idle so user can record again
        await invoke('reset_to_idle');
        return;
      }

      const filePath = await stopRecording();

      console.log('[Recording] Saved to:', filePath, 'Duration:', duration?.toFixed(1), 's');

      onRecordingComplete?.({
        filePath,
        duration,
      });

      // Trigger transcription pipeline
      console.log('[Recording] Starting transcription pipeline...');
      invoke('process_audio', { audioPath: filePath })
        .then((result) => {
          console.log('[Recording] Transcription complete:', result);
        })
        .catch((error) => {
          console.error('[Recording] Transcription failed:', error);
          onError?.(String(error));
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
