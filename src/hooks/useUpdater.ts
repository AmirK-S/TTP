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
}

export function useUpdater(options?: UseUpdaterOptions) {
  const { autoCheck = false } = options ?? {};

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

  const restartApp = useCallback(async () => {
    await relaunch();
  }, []);

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
