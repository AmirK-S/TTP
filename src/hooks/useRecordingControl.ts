// TTP - Talk To Paste
// Hook to control actual microphone recording via tauri-plugin-mic-recorder
// This hooks into the recording-state-changed events from Rust and
// starts/stops the mic recording plugin accordingly.

import { startRecording, stopRecording } from 'tauri-plugin-mic-recorder-api';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { useRef, useCallback } from 'react';
import { useTauriEvent } from './useTauriEvent';

type RecordingState = 'Idle' | 'Recording' | 'Processing';

interface RecordingResult {
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
    } catch (error) {
      isRecordingRef.current = false;
      recordingStartTime.current = null;
      const errorMsg = String(error);
      // Surface the failure to FloatingBar via the same event the Rust pipeline uses,
      // so the user always sees feedback even when no `onError` handler is wired.
      const friendly = /permission/i.test(errorMsg)
        ? 'Microphone permission denied — check System Settings'
        : `Mic error: ${errorMsg.slice(0, 120)}`;
      emit('transcription-progress', { stage: 'error', message: friendly }).catch(() => {});
      onError?.(errorMsg);
      // Reset Rust state to Idle so the user can record again
      invoke('reset_to_idle').catch(() => {});
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

      onRecordingComplete?.({
        filePath,
        duration,
      });

      // Trigger transcription pipeline
      invoke('process_audio', { audioPath: filePath })
        .catch((error) => {
          onError?.(String(error));
        });
    } catch (error) {
      recordingStartTime.current = null;
      onError?.(String(error));
      // Reset Rust state to Idle so the user can record again
      invoke('reset_to_idle').catch(() => {});
    }
  }, [onRecordingComplete, onError]);

  // Listen for recording state changes from Rust. The handler reads
  // handleStart/Stop via the ref-stable hook, so we register exactly once
  // for the lifetime of the component instead of churning on every render
  // — that churn was the suspected cause of TTP-5.
  useTauriEvent<RecordingState>('recording-state-changed', async (event) => {
    const state = event.payload;
    if (state === 'Recording' && !isRecordingRef.current) {
      await handleStartRecording();
    } else if (state === 'Processing') {
      if (isRecordingRef.current) {
        await handleStopRecording();
      } else {
        invoke('reset_to_idle').catch(() => {});
      }
    }
  });

  return {
    isRecording: isRecordingRef.current,
  };
}
