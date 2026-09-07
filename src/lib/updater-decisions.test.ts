import { describe, it, expect } from 'vitest';
import {
  scrubUpdateError,
  shouldNotifyUpdate,
  shouldResetDismissOnVersionChange,
  shouldAutoInstall,
  shouldAutoRestart,
} from './updater-decisions';

describe('scrubUpdateError', () => {
  // The regex replaces ONLY the `/Users/<name>` and `/home/<name>` segments,
  // leaving the rest of the path intact. That preserves enough context to
  // triage (which subdirectory failed) without leaking the username.
  it('replaces /Users/<name> with [USER], keeps subpath', () => {
    expect(scrubUpdateError('Failed to write /Users/alice/Library/foo'))
      .toBe('Failed to write [USER]/Library/foo');
  });

  it('replaces /home/<name> on Linux, keeps subpath', () => {
    expect(scrubUpdateError('Failed to write /home/bob/.config/foo'))
      .toBe('Failed to write [USER]/.config/foo');
  });

  it('truncates at 200 chars to cap telemetry payload size', () => {
    const long = 'x'.repeat(500);
    expect(scrubUpdateError(long).length).toBe(200);
  });

  it('leaves short generic errors unchanged', () => {
    expect(scrubUpdateError('network timeout')).toBe('network timeout');
  });

  it('handles multiple usernames in one message', () => {
    const out = scrubUpdateError('move /Users/a/foo to /Users/b/bar');
    expect(out).not.toContain('/Users/a');
    expect(out).not.toContain('/Users/b');
    expect(out.match(/\[USER\]/g)?.length).toBe(2);
  });
});

describe('shouldNotifyUpdate', () => {
  it('notifies when info present, idle, not dismissed', () => {
    expect(shouldNotifyUpdate(true, 'Idle', false)).toBe(true);
  });

  it('does not notify when no update info', () => {
    expect(shouldNotifyUpdate(false, 'Idle', false)).toBe(false);
  });

  it('does not notify while recording', () => {
    expect(shouldNotifyUpdate(true, 'Recording', false)).toBe(false);
  });

  it('does not notify while processing', () => {
    expect(shouldNotifyUpdate(true, 'Processing', false)).toBe(false);
  });

  it('does not notify when dismissed', () => {
    expect(shouldNotifyUpdate(true, 'Idle', true)).toBe(false);
  });
});

describe('shouldResetDismissOnVersionChange', () => {
  it('resets on first ever surface (no previous)', () => {
    expect(shouldResetDismissOnVersionChange(null, '3.0.5')).toBe(true);
  });

  it('resets when new version differs', () => {
    expect(shouldResetDismissOnVersionChange('3.0.5', '3.0.6')).toBe(true);
  });

  it('does NOT reset when same version found again', () => {
    expect(shouldResetDismissOnVersionChange('3.0.5', '3.0.5')).toBe(false);
  });
});

describe('shouldAutoInstall', () => {
  const opts = (over: Partial<Parameters<typeof shouldAutoInstall>[number]> = {}) => ({
    autoInstall: true,
    status: 'available' as const,
    recordingState: 'Idle' as const,
    alreadyInstalledThisSession: false,
    ...over,
  });

  it('fires when all guards pass', () => {
    expect(shouldAutoInstall(true, 'available', 'Idle', false)).toBe(true);
  });

  it('skips when auto-install is opted out', () => {
    expect(shouldAutoInstall(false, 'available', 'Idle', false)).toBe(false);
  });

  it('skips when no update is available yet', () => {
    expect(shouldAutoInstall(true, 'checking', 'Idle', false)).toBe(false);
    expect(shouldAutoInstall(true, 'idle', 'Idle', false)).toBe(false);
    expect(shouldAutoInstall(true, 'up-to-date', 'Idle', false)).toBe(false);
  });

  it('skips during active recording', () => {
    expect(shouldAutoInstall(true, 'available', 'Recording', false)).toBe(false);
    expect(shouldAutoInstall(true, 'available', 'Processing', false)).toBe(false);
  });

  it('skips when already installed this session (the 4h-cycle re-install guard)', () => {
    expect(shouldAutoInstall(true, 'available', 'Idle', true)).toBe(false);
  });
});

describe('shouldAutoRestart', () => {
  it('fires when ready, idle, and no recording since', () => {
    expect(shouldAutoRestart(true, 'ready', 'Idle', false)).toBe(true);
  });

  it('skips when auto-install is opted out', () => {
    expect(shouldAutoRestart(false, 'ready', 'Idle', false)).toBe(false);
  });

  it('skips before the install completes', () => {
    expect(shouldAutoRestart(true, 'downloading', 'Idle', false)).toBe(false);
    expect(shouldAutoRestart(true, 'available', 'Idle', false)).toBe(false);
  });

  it('skips during active recording', () => {
    expect(shouldAutoRestart(true, 'ready', 'Recording', false)).toBe(false);
  });

  it('skips once the user has recorded since install completed', () => {
    // This is the v2.1.9 fix: an active user must not be yanked out by an
    // auto-relaunch. They'll pick up the new bundle on their next quit.
    expect(shouldAutoRestart(true, 'ready', 'Idle', true)).toBe(false);
  });
});
