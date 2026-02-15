// TTP - Talk To Paste
// Hook for listening to recording state changes from Rust backend

import { listen } from '@tauri-apps/api/event';
import { useState, useEffect } from 'react';

export type RecordingState = 'Idle' | 'Recording' | 'Processing';

/**
 * Hook to subscribe to recording state changes from the Rust backend.
 * Returns the current recording state which updates automatically when
 * the backend emits 'recording-state-changed' events.
 */
export function useRecordingState() {
  const [state, setState] = useState<RecordingState>('Idle');

  useEffect(() => {
    const unlisten = listen<RecordingState>('recording-state-changed', (event) => {
      setState(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return state;
}
