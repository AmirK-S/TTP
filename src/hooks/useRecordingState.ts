// TTP - Talk To Paste
// Hook for listening to recording state changes from Rust backend

import { useState } from 'react';
import { useTauriEvent } from './useTauriEvent';

export type RecordingState = 'Idle' | 'Recording' | 'Processing';

/**
 * Hook to subscribe to recording state changes from the Rust backend.
 * Returns the current recording state which updates automatically when
 * the backend emits 'recording-state-changed' events.
 */
export function useRecordingState() {
  const [state, setState] = useState<RecordingState>('Idle');

  useTauriEvent<RecordingState>('recording-state-changed', (event) => {
    setState(event.payload);
  });

  return state;
}
