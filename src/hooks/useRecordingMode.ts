// TTP - Talk To Paste
// Hook that reflects the active recording modality (push-to-talk vs
// hands-free toggle) so the FloatingBar can render a hands-free affordance.
//
// Rust emits this on every Recording-state entry/exit:
//   * "toggle"        — recording will end on a single Fn tap.
//   * "push_to_talk"  — recording ends on key release.
//   * null            — no recording in progress.
//
// The mode is computed from `AppState::effective_hands_free()` at the
// instant Recording starts, so it correctly accounts for both:
//   * the persisted Settings preference, and
//   * the transient `session_hands_free` override (Fn double-tap, tray Start).

import { useState } from 'react';
import { useTauriEvent } from './useTauriEvent';

export type RecordingMode = 'toggle' | 'push_to_talk' | null;

export function useRecordingMode(): RecordingMode {
  const [mode, setMode] = useState<RecordingMode>(null);
  useTauriEvent<RecordingMode>('recording-mode-changed', (event) => {
    setMode(event.payload);
  });
  return mode;
}
