// TTP - Talk To Paste
// Pure decision helpers extracted from useUpdater.ts.
//
// useUpdater (370 LOC) mixes IPC calls, timer lifecycle, React refs, and
// derived state. The IPC + lifecycle bits are testable only via heavy
// integration harness, but the DECISION bits — "should we notify?",
// "is this a new version that should reset dismiss?", "did the user
// record yet?" — are pure functions that the audit explicitly flagged as
// untested. Extracting them here makes them callable from unit tests
// without renderHook + Tauri event mocks.

/**
 * Strip user-identifying paths from an update error message before sending
 * it via `update_failed` telemetry. The Rust side has its own regex
 * scrubber (`src/lib/sentry.ts::scrubMessage`); this one focuses on the
 * specific `/Users/<name>` and `/home/<name>` patterns that download
 * failures often carry.
 */
export function scrubUpdateError(msg: string): string {
  return msg
    .replace(/\/Users\/[^\s/"']+/g, '[USER]')
    .replace(/\/home\/[^\s/"']+/g, '[USER]')
    .slice(0, 200);
}

/**
 * Decide whether to surface the "Update available" UI to the user.
 *
 * Three guards:
 *   1. We must actually have version info (the check found something).
 *   2. The user must be idle — never interrupt an active recording with a
 *      modal. The pill window is small and an update banner would compete
 *      for the same screen real estate.
 *   3. The user must not have already dismissed this version.
 */
export function shouldNotifyUpdate(
  hasUpdateInfo: boolean,
  recordingState: 'Idle' | 'Recording' | 'Processing',
  dismissed: boolean,
): boolean {
  return hasUpdateInfo && recordingState === 'Idle' && !dismissed;
}

/**
 * Returns true if the newly-found version is different from the one we
 * previously surfaced. Used to clear the "dismissed" flag when a brand
 * new version drops, so a user who dismissed v3.0.5 still sees v3.0.6.
 *
 * Null `previousVersion` means "no prior surface" — anything is new.
 */
export function shouldResetDismissOnVersionChange(
  previousVersion: string | null,
  newVersion: string,
): boolean {
  if (previousVersion === null) return true;
  return previousVersion !== newVersion;
}

/**
 * Should the auto-install trigger fire?
 *
 * The audit flagged `autoInstalledThisSessionRef` as load-bearing: Tauri's
 * update plugin compares the manifest with the RUNNING process, not the
 * on-disk bundle. After a silent install, the running process is still
 * the old version — so subsequent 4h checks keep reporting "available"
 * and we'd re-download + re-install the same update every cycle.
 *
 * Guards:
 *   1. Auto-install must be opted in.
 *   2. Status must be 'available' (the check succeeded with a new version).
 *   3. Recording state must be 'Idle' — never download mid-recording.
 *   4. We must not have already installed this session.
 */
export function shouldAutoInstall(
  autoInstall: boolean,
  status: 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date',
  recordingState: 'Idle' | 'Recording' | 'Processing',
  alreadyInstalledThisSession: boolean,
): boolean {
  if (!autoInstall) return false;
  if (status !== 'available') return false;
  if (recordingState !== 'Idle') return false;
  if (alreadyInstalledThisSession) return false;
  return true;
}

/**
 * Should the auto-restart-after-install timer arm?
 *
 * The v2.1.6 60s idle timer was naive: it would arm on idle and a user
 * who opened TTP, did something else for ~60s, then pressed Fn would hit
 * the restart firing right as they began recording. v2.1.9 added the
 * "user must not have recorded this session" gate.
 *
 * Guards:
 *   1. Auto-install opted in.
 *   2. Install completed (status === 'ready').
 *   3. Recording state Idle.
 *   4. User has not recorded since the install completed.
 */
export function shouldAutoRestart(
  autoInstall: boolean,
  status: 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date',
  recordingState: 'Idle' | 'Recording' | 'Processing',
  hasRecordedSinceReady: boolean,
): boolean {
  if (!autoInstall) return false;
  if (status !== 'ready') return false;
  if (recordingState !== 'Idle') return false;
  if (hasRecordedSinceReady) return false;
  return true;
}
