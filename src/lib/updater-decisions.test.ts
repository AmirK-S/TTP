import { describe, it, expect } from 'vitest';
import {
  shouldNotifyUpdate,
  shouldResetDismissOnVersionChange,
  shouldAutoInstall,
  shouldAutoRestart,
} from './updater-decisions';

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
  it('fires when staged and idle', () => {
    expect(shouldAutoRestart(true, 'ready', 'Idle')).toBe(true);
  });

  it('skips when auto-install is opted out', () => {
    expect(shouldAutoRestart(false, 'ready', 'Idle')).toBe(false);
  });

  it('skips before the download completes', () => {
    expect(shouldAutoRestart(true, 'downloading', 'Idle')).toBe(false);
    expect(shouldAutoRestart(true, 'available', 'Idle')).toBe(false);
  });

  it('skips during a recording or while it is processed', () => {
    expect(shouldAutoRestart(true, 'ready', 'Recording')).toBe(false);
    expect(shouldAutoRestart(true, 'ready', 'Processing')).toBe(false);
  });
});
