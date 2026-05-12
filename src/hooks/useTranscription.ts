// TTP - Talk To Paste
// Hook for listening to transcription progress events from Rust backend

import { useEffect, useRef, useState } from 'react';
import { useTauriEvent } from './useTauriEvent';

type TranscriptionStage =
  | 'idle'
  | 'transcribing'
  | 'polishing'
  | 'pasting'
  | 'complete'
  | 'error';

interface TranscriptionProgress {
  stage: TranscriptionStage;
  // After the i18n refactor, `message` is a translation key emitted by Rust
  // (e.g. 'error.no_speech', 'progress.transcribing') or '' when there is no
  // text to display. Legacy/unknown values are displayed as-is by consumers.
  message: string;
  // Optional interpolation values for the translation key.
  params?: Record<string, string | number>;
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
  const [params, setParams] = useState<Record<string, string | number> | undefined>(undefined);
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
    };
  }, []);

  useTauriEvent<TranscriptionProgress>('transcription-progress', (event) => {
    setStage(event.payload.stage);
    setMessage(event.payload.message);
    setParams(event.payload.params);

    if (resetTimerRef.current) {
      clearTimeout(resetTimerRef.current);
      resetTimerRef.current = null;
    }

    if (event.payload.stage === 'complete') {
      resetTimerRef.current = setTimeout(() => {
        setStage('idle');
        setMessage('');
        setParams(undefined);
        resetTimerRef.current = null;
      }, 500);
    } else if (event.payload.stage === 'error') {
      resetTimerRef.current = setTimeout(() => {
        setStage('idle');
        setMessage('');
        setParams(undefined);
        resetTimerRef.current = null;
      }, 4000);
    }
  });

  const isProcessing = stage !== 'idle';

  return { stage, message, params, isProcessing };
}
