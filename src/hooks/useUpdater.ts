// TTP - Talk To Paste
// Hook for checking and installing app updates
// Supports automatic periodic checking, idle-state gating, and accurate download progress

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { useState, useCallback, useEffect, useRef } from 'react';
import { trackEvent } from '../lib/analytics';
import { useRecordingState } from './useRecordingState';
import { useSettingsStore } from '../stores/settings-store';

/**
 * Channel-aware update check result returned by the Rust IPC.
 * Mirrors the `UpdateCheckResult` enum in src-tauri/src/lib.rs (serde tag = "kind").
 */
type UpdateCheckResultPayload =
  | { kind: 'available'; version: string; body: string | null }
  | { kind: 'no-update' };

interface UpdateProgressPayload {
  downloaded: number;
  total: number | null;
}

type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date';

interface UpdateInfo {
  version: string;
  body?: string;
}

const UPDATE_INTERVAL_MS = 4 * 60 * 60 * 1000; // 4 hours


/// Strip user-identifying paths and truncate, so update_failed telemetry stays safe.
function scrubUpdateError(msg: string): string {
  return msg
    .replace(/\/Users\/[^\s/"']+/g, '[USER]')
    .replace(/\/home\/[^\s/"']+/g, '[USER]')
    .slice(0, 200);
}

interface UseUpdaterOptions {
  autoCheck?: boolean;
  /// When true, the hook will silently call `downloadAndInstall()` as soon
  /// as `checkForUpdates()` reports a new version available (gated on
  /// recordingState === 'Idle'). Used by the main App so a menu-bar user
  /// who never opens the window still gets updates — install completes in
  /// the background and the tray shows a blue dot + "Install update" menu
  /// item nudging the user to relaunch when convenient.
  autoInstall?: boolean;
}

export function useUpdater(options?: UseUpdaterOptions) {
  const { autoCheck = false, autoInstall = false } = options ?? {};

  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [progress, setProgress] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  // Track the last found version so dismiss resets on new version
  const lastFoundVersionRef = useRef<string | null>(null);

  // Tracks the "drop status back to idle after error" timer so we can clear it on unmount.
  const idleResetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Hold the unlisten functions for the in-flight progress listeners so we can
  // tear them down even if React unmounts mid-download.
  const progressUnlistenRef = useRef<UnlistenFn | null>(null);
  const finishUnlistenRef = useRef<UnlistenFn | null>(null);

  // Tauri's update check compares the manifest version with the *running*
  // process version, not the on-disk bundle. After a silent install the
  // running process is still old, so subsequent 4h checks would keep
  // reporting "available" and we'd re-download + re-install the same
  // update every cycle. This ref guards against that — once we've
  // successfully installed in this session, we stop auto-triggering until
  // the user relaunches.
  const autoInstalledThisSessionRef = useRef(false);

  // Read the user's channel preference. We pull it on demand at click time
  // (from the store's getState) so that toggling beta does not require the
  // hook to re-run; this also keeps the hook usable from windows that don't
  // call loadSettings().
  const useBetaChannel = useSettingsStore((s) => s.useBetaChannel);
  const useBetaChannelRef = useRef(useBetaChannel);
  useEffect(() => {
    useBetaChannelRef.current = useBetaChannel;
  }, [useBetaChannel]);

  const scheduleIdleReset = useCallback((delayMs: number) => {
    if (idleResetTimerRef.current) clearTimeout(idleResetTimerRef.current);
    idleResetTimerRef.current = setTimeout(() => {
      setStatus('idle');
      idleResetTimerRef.current = null;
    }, delayMs);
  }, []);

  useEffect(() => {
    return () => {
      if (idleResetTimerRef.current) clearTimeout(idleResetTimerRef.current);
      if (progressUnlistenRef.current) progressUnlistenRef.current();
      if (finishUnlistenRef.current) finishUnlistenRef.current();
    };
  }, []);

  const recordingState = useRecordingState();

  // Derived: should we notify the user about the update?
  const shouldNotify = updateInfo !== null && recordingState === 'Idle' && !dismissed;

  const checkForUpdates = useCallback(async () => {
    setStatus('checking');
    setError(null);

    try {
      const result = await invoke<UpdateCheckResultPayload>(
        'check_for_updates_with_channel',
        { useBeta: useBetaChannelRef.current }
      );

      if (result.kind === 'available') {
        setUpdateInfo({
          version: result.version,
          body: result.body ?? undefined,
        });
        setStatus('available');

        // Reset dismissed state if this is a new version
        if (lastFoundVersionRef.current !== result.version) {
          lastFoundVersionRef.current = result.version;
          setDismissed(false);
        }

        getVersion()
          .then((currentVersion) => {
            trackEvent('update_prompted', {
              from_version: currentVersion,
              to_version: result.version,
            });
          })
          .catch(() => {});
        return true;
      }

      setStatus('up-to-date');
      // Reset to idle after 5 seconds so the button reappears
      scheduleIdleReset(5000);
      return false;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error('[Updater] Check failed:', msg);
      trackEvent('update_failed', {
        stage: 'check',
        error: scrubUpdateError(msg),
      });
      setError(msg);
      setStatus('error');
      // Drop back to idle so the user can hit "Check" again — without this
      // the button stayed disabled forever after a transient network blip.
      scheduleIdleReset(5000);
      return false;
    }
  }, [scheduleIdleReset]);

  // Periodic auto-check: on mount + every 4 hours (only when autoCheck=true)
  useEffect(() => {
    if (!autoCheck) return;

    // Check on mount (launch)
    checkForUpdates();

    const intervalId = setInterval(() => {
      checkForUpdates();
    }, UPDATE_INTERVAL_MS);

    return () => clearInterval(intervalId);
  }, [autoCheck, checkForUpdates]);

  const downloadAndInstall = useCallback(async () => {
    setStatus('downloading');
    setProgress(0);

    let totalBytes: number | null = null;
    let downloadedBytes = 0;

    try {
      // Subscribe to progress events emitted by the Rust IPC for this install.
      // The plugin streams chunk-level progress so the percentage UX stays accurate.
      progressUnlistenRef.current = await listen<UpdateProgressPayload>(
        'update-progress',
        (event) => {
          downloadedBytes = event.payload.downloaded;
          if (event.payload.total && event.payload.total > 0) {
            totalBytes = event.payload.total;
          }
          if (totalBytes && totalBytes > 0) {
            setProgress(Math.min(Math.round((downloadedBytes / totalBytes) * 100), 99));
          }
        }
      );

      finishUnlistenRef.current = await listen('update-progress-finished', () => {
        setProgress(100);
      });

      const installedVersion = await invoke<string>('install_update_with_channel', {
        useBeta: useBetaChannelRef.current,
      });

      // Tear down listeners — the install is done.
      if (progressUnlistenRef.current) {
        progressUnlistenRef.current();
        progressUnlistenRef.current = null;
      }
      if (finishUnlistenRef.current) {
        finishUnlistenRef.current();
        finishUnlistenRef.current = null;
      }

      setStatus('ready');

      // Notify the Rust side so the tray surfaces the "Install update" menu
      // item with a blue-dot icon overlay. Best-effort — if this fails the
      // in-app "Restart" prompt still works, the user just loses the tray
      // shortcut.
      try {
        await invoke('mark_update_ready', { version: installedVersion });
      } catch (e) {
        console.warn('[Updater] mark_update_ready failed:', e);
      }

      getVersion()
        .then((currentVersion) => {
          trackEvent('update_completed', {
            from_version: currentVersion,
            to_version: installedVersion,
          });
        })
        .catch(() => {});
    } catch (e) {
      // Tear down listeners if the IPC threw mid-flight.
      if (progressUnlistenRef.current) {
        progressUnlistenRef.current();
        progressUnlistenRef.current = null;
      }
      if (finishUnlistenRef.current) {
        finishUnlistenRef.current();
        finishUnlistenRef.current = null;
      }

      const msg = e instanceof Error ? e.message : String(e);
      console.error('[Updater] Download failed:', msg);
      trackEvent('update_failed', {
        stage: 'download',
        error: scrubUpdateError(msg),
      });
      setError(msg);
      setStatus('error');
      scheduleIdleReset(5000);
    }
  }, [scheduleIdleReset]);

  // Silent auto-install: once a check reports a version available and the
  // user is idle, immediately download + install in the background. The
  // tray then exposes the "Install update" menu item via mark_update_ready.
  // The ref guard prevents re-installing on every 4h cycle (Tauri compares
  // the manifest against the running process, which is still old until
  // the user actually relaunches).
  useEffect(() => {
    if (!autoInstall) return;
    if (status !== 'available') return;
    if (recordingState !== 'Idle') return;
    if (autoInstalledThisSessionRef.current) return;
    autoInstalledThisSessionRef.current = true;
    downloadAndInstall();
  }, [autoInstall, status, recordingState, downloadAndInstall]);

  // If the auto-install attempt errored (network blip, disk full, etc.),
  // clear the guard so the next 4h check can retry. We deliberately do
  // not clear it on success — once installed, the running process is
  // still old and would otherwise re-install the same update every cycle.
  useEffect(() => {
    if (status === 'error') {
      autoInstalledThisSessionRef.current = false;
    }
  }, [status]);

  const restartApp = useCallback(async () => {
    // Prefer the Rust-side restart command which uses LaunchServices on
    // macOS (`open -n -a`) — the plugin-process relaunch flow has been
    // silently failing on beta builds: the current process dies but
    // Gatekeeper / LaunchServices won't surface the new one, leaving
    // users with the app simply gone. The custom command sidesteps the
    // exec chain entirely.
    try {
      await invoke('restart_app_post_update');
    } catch (e) {
      console.error('[Updater] restart_app_post_update failed, falling back to plugin-process relaunch:', e);
      await relaunch();
    }
  }, []);

  // Once the user has actually used the app this session (started at least
  // one recording), we lock auto-restart OFF for the rest of the session.
  // The 60s grace timer in v2.1.6 was naive: it would arm on idle, and a
  // user who opened TTP, did something else for ~60s, then pressed Fn would
  // hit the restart firing exactly when they began recording. The fix is to
  // commit: if the user is using the app, defer the relaunch to the next
  // natural quit-then-open — they'll pick up the new bundle then. We trust
  // the tray "Install update (vX.Y.Z)" menu item (set by mark_update_ready)
  // as the explicit nudge for users who want to relaunch on their own.
  const hasRecordedSinceReadyRef = useRef(false);
  useEffect(() => {
    if (recordingState === 'Recording') {
      hasRecordedSinceReadyRef.current = true;
    }
  }, [recordingState]);

  // Auto-relaunch right after a silent install completes. The window
  // between "install done" and "user clicks Restart" leaves the running
  // process in a zombie state: old code in RAM, new bundle on disk —
  // macOS can revoke the mic TCC grant (bundle signature changed under
  // the process) and the audio plugin's lazy-loaded resources point at
  // files that no longer match. Symptom users hit: pill shows up on
  // hotkey press but no audio reaches transcription.
  //
  // Two gates before we restart:
  // 1. recordingState must be Idle (never yank an active recording).
  // 2. The user must NOT have recorded yet this session. If they have,
  //    they're actively using the app and any restart we fire is an
  //    interruption regardless of timing. They'll get the new bundle on
  //    their next quit/relaunch — that's good enough.
  //
  // The 60s timer is kept as a safety net for the genuinely-idle case
  // (user opened TTP, never used it, walked away). It gets cancelled by
  // cleanup if anything else fires the effect first.
  useEffect(() => {
    if (!autoInstall) return;
    if (status !== 'ready') return;
    if (recordingState !== 'Idle') return;
    if (hasRecordedSinceReadyRef.current) return;
    const timer = setTimeout(() => {
      restartApp();
    }, 60_000);
    return () => clearTimeout(timer);
  }, [autoInstall, status, recordingState, restartApp]);

  const dismiss = useCallback(() => {
    setDismissed(true);
  }, []);

  return {
    status,
    updateInfo,
    progress,
    error,
    checkForUpdates,
    downloadAndInstall,
    restartApp,
    shouldNotify,
    dismiss,
  };
}
