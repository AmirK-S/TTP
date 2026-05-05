// TTP - Talk To Paste
// Hook for listening to transcription progress events from Rust backend

import { useState } from 'react';
import { useTauriEvent } from './useTauriEvent';

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
 * - polishing: Text being cleaned by Groq LLM
 * - pasting: Text being pasted into active app
 * - complete: Pipeline finished successfully
 * - error: Pipeline encountered an error
 */
export function useTranscription() {
  const [stage, setStage] = useState<TranscriptionStage>('idle');
  const [message, setMessage] = useState('');

  useTauriEvent<TranscriptionProgress>('transcription-progress', (event) => {
    setStage(event.payload.stage);
    setMessage(event.payload.message);

    if (event.payload.stage === 'complete') {
      setTimeout(() => {
        setStage('idle');
        setMessage('');
      }, 500);
    } else if (event.payload.stage === 'error') {
      setTimeout(() => {
        setStage('idle');
        setMessage('');
      }, 4000);
    }
  });

  const isProcessing = stage !== 'idle';

  return { stage, message, isProcessing };
}
