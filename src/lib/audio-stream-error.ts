// TTP - Talk To Paste
// Pure helper to interpret the `audio-stream-error` event payload.
//
// History: until v3.1 the Rust side emitted a bare `string` payload
// regardless of whether the capture stream (data loss) or the monitor
// stream (waveform died, recording fine) was the source. The frontend
// reacted identically — abort + toast — even when the actual recording
// would have completed successfully. v3.1 changed Rust to emit
// `{ source, message }` and this helper interprets the two shapes.
//
// String payloads are still possible from a stale Rust build that ships
// before the renderer updates (or after a downgrade). We treat them as
// `capture` since that was the only emitter pre-v3.1.

export type AudioStreamErrorSource = 'capture' | 'monitor';

export interface ParsedAudioStreamError {
  /** The stream that died. Used to route capture vs monitor failures. */
  source: AudioStreamErrorSource;
  /** Free-form detail for logging; empty when none was provided. */
  detail: string;
  /** True when the capture stream failed — the caller must abort + toast. */
  isCaptureFailure: boolean;
}

export function parseAudioStreamError(
  payload: string | { source?: 'capture' | 'monitor'; message?: string } | null | undefined,
): ParsedAudioStreamError {
  // String payload — legacy Rust build, treat as capture failure.
  if (typeof payload === 'string') {
    return { source: 'capture', detail: payload, isCaptureFailure: true };
  }

  // Object payload — the v3.1 shape. Default to capture when the source
  // field is absent (defensive: an attacker who can fake the event would
  // pick monitor to silence the error; defaulting to capture is the safer
  // choice since it forces a visible abort).
  if (payload && typeof payload === 'object') {
    const source: AudioStreamErrorSource = payload.source === 'monitor' ? 'monitor' : 'capture';
    return {
      source,
      detail: payload.message ?? '',
      isCaptureFailure: source === 'capture',
    };
  }

  // Null / undefined — treat as capture failure.
  return { source: 'capture', detail: '', isCaptureFailure: true };
}
