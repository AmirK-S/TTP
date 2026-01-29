// TTP - Talk To Paste
// Hook for listening to transcription progress events from Rust backend

import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';

export type TranscriptionStage =
  | 'idle'
  | 'transcribing'
  | 'polishing'
  | 'pasting'
  | 'complete'
  | 'error';

interface TranscriptionProgress {
  stage: TranscriptionStage;
  message: string;
}

/**
 * Hook to subscribe to transcription progress events from the Rust backend.
 * Returns the current stage, message, and whether processing is in progress.
 *
 * Stages:
 * - idle: No transcription in progress
 * - transcribing: Audio being transcribed by Whisper
 * - polishing: Text being cleaned by GPT-4o-mini
 * - pasting: Text being pasted into active app
 * - complete: Pipeline finished successfully
 * - error: Pipeline encountered an error
 */
export function useTranscription() {
  const [stage, setStage] = useState<TranscriptionStage>('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    const unlisten = listen<TranscriptionProgress>('transcription-progress', (event) => {
      setStage(event.payload.stage);
      setMessage(event.payload.message);

      // Reset to idle after complete/error with a short delay
      if (event.payload.stage === 'complete' || event.payload.stage === 'error') {
        setTimeout(() => {
          setStage('idle');
          setMessage('');
        }, 500);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const isProcessing = stage !== 'idle';

  return { stage, message, isProcessing };
}
