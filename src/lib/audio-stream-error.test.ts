import { describe, it, expect } from 'vitest';
import { parseAudioStreamError } from './audio-stream-error';

describe('parseAudioStreamError', () => {
  it('parses the v3.1 capture payload as a capture failure', () => {
    const out = parseAudioStreamError({ source: 'capture', message: 'cpal stream error' });
    expect(out.source).toBe('capture');
    expect(out.detail).toBe('cpal stream error');
    expect(out.isCaptureFailure).toBe(true);
  });

  it('parses the v3.1 monitor payload as NOT a capture failure', () => {
    // The point of disambiguation: a monitor failure should not yank the
    // recording session — the user keeps speaking and the WAV keeps writing,
    // the waveform just goes flat.
    const out = parseAudioStreamError({ source: 'monitor', message: 'rms thread died' });
    expect(out.source).toBe('monitor');
    expect(out.isCaptureFailure).toBe(false);
  });

  it('falls back to capture when source field is missing', () => {
    // Defensive: a renderer talking to a Rust build that emits a partial
    // shape should default to the safer (more visible) failure mode.
    const out = parseAudioStreamError({ message: 'mystery' });
    expect(out.source).toBe('capture');
    expect(out.isCaptureFailure).toBe(true);
  });

  it('treats a string payload as a capture failure (legacy Rust)', () => {
    const out = parseAudioStreamError('cpal stream error: device disconnected');
    expect(out.source).toBe('capture');
    expect(out.detail).toBe('cpal stream error: device disconnected');
    expect(out.isCaptureFailure).toBe(true);
  });

  it('treats null/undefined as capture failure with empty detail', () => {
    expect(parseAudioStreamError(null)).toEqual({
      source: 'capture',
      detail: '',
      isCaptureFailure: true,
    });
    expect(parseAudioStreamError(undefined)).toEqual({
      source: 'capture',
      detail: '',
      isCaptureFailure: true,
    });
  });

  it('coerces unknown source values to capture (no silent passthrough)', () => {
    const out = parseAudioStreamError({
      source: 'weird-future-value' as 'capture' | 'monitor',
      message: 'x',
    });
    expect(out.source).toBe('capture');
    expect(out.isCaptureFailure).toBe(true);
  });

  it('returns empty detail when message field is absent', () => {
    const out = parseAudioStreamError({ source: 'monitor' });
    expect(out.detail).toBe('');
  });
});
