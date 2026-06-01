// TTP - Talk To Paste
// Hook to control actual microphone recording via our in-house cpal recorder
// (src-tauri/src/audio_capture.rs). Replaces the upstream
// tauri-plugin-mic-recorder, which silently dropped samples and swallowed
// stream errors — see audio_capture.rs module-level comment.
//
// Hooks into the recording-state-changed events from Rust and
// starts/stops capture accordingly.

import { invoke } from '@tauri-apps/api/core';

const startRecording = () => invoke<void>('start_recording');
const stopRecording = () => invoke<string>('stop_recording');
import { emit } from '@tauri-apps/api/event';
import { useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
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
  const { t } = useTranslation();
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
        ? t('error.microphone_permission_denied')
        : t('error.microphone_generic', { error: errorMsg.slice(0, 120) });
      emit('transcription-progress', { stage: 'error', message: friendly }).catch(() => {});
      onError?.(errorMsg);
      // Reset Rust state to Idle so the user can record again
      invoke('reset_to_idle').catch(() => {});
    }
  }, [onError, t]);

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

  // Audio stream got kicked out of the mic (e.g. F5/macOS Dictation took exclusive
  // access). cpal can't recover from inside its error callback, so the Rust side
  // emits this event; we cancel the in-flight recording, reset state, and surface
  // a user-facing message via the same error pill that the rest of the pipeline uses.
  // No toast lib in TTP yet, so we ride on `transcription-progress` which the
  // FloatingBar already renders as a red pill auto-dismissing after 4s.
  useTauriEvent<string>('audio-stream-error', (event) => {
    console.warn('[AudioStream] Stream error from Rust:', event.payload);
    isRecordingRef.current = false;
    recordingStartTime.current = null;
    // Best-effort: try to stop the mic-recorder plugin so it releases the file
    // handle. It may already be in an error state — ignore.
    stopRecording().catch(() => {});
    invoke('reset_to_idle').catch(() => {});
    emit('transcription-progress', {
      stage: 'error',
      message: t('error.audio_stream_interrupted'),
    }).catch(() => {});
  });

  return {
    isRecording: isRecordingRef.current,
  };
}
