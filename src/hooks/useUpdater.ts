// TTP - Talk To Paste
// Hook for checking and installing app updates
// Supports automatic periodic checking, idle-state gating, and accurate download progress

import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { useState, useCallback, useEffect, useRef } from 'react';
import { trackEvent } from '../lib/analytics';
import { useRecordingState } from './useRecordingState';

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error';

export interface UpdateInfo {
  version: string;
  body?: string;
}

const UPDATE_INTERVAL_MS = 4 * 60 * 60 * 1000; // 4 hours

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

  // Store the actual Update object from the plugin for reuse in downloadAndInstall
  const pendingUpdateRef = useRef<Update | null>(null);

  // Track the last found version so dismiss resets on new version
  const lastFoundVersionRef = useRef<string | null>(null);

  const recordingState = useRecordingState();

  // Derived: should we notify the user about the update?
  const shouldNotify = updateInfo !== null && recordingState === 'Idle' && !dismissed;

  const checkForUpdates = useCallback(async () => {
    setStatus('checking');
    setError(null);

    try {
      const update = await check();

      if (update) {
        // Store the Update object for later use in downloadAndInstall
        pendingUpdateRef.current = update;

        setUpdateInfo({
          version: update.version,
          body: update.body,
        });
        setStatus('available');

        // Reset dismissed state if this is a new version
        if (lastFoundVersionRef.current !== update.version) {
          lastFoundVersionRef.current = update.version;
          setDismissed(false);
        }

        getVersion().then(currentVersion => {
          trackEvent("update_prompted", {
            from_version: currentVersion,
            to_version: update.version,
          });
        }).catch(() => {});
        return true;
      } else {
        setStatus('idle');
        return false;
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error('[Updater] Check failed:', msg);
      setError(msg);
      setStatus('error');
      return false;
    }
  }, []);

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
    const update = pendingUpdateRef.current;
    if (!update) {
      console.error('[Updater] No pending update to download');
      setStatus('idle');
      return;
    }

    setStatus('downloading');
    setProgress(0);

    let totalBytes = 0;
    let downloadedBytes = 0;

    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          totalBytes = event.data.contentLength ?? 0;
          downloadedBytes = 0;
          setProgress(0);
        } else if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength;
          if (totalBytes > 0) {
            setProgress(Math.min(Math.round((downloadedBytes / totalBytes) * 100), 99));
          }
        } else if (event.event === 'Finished') {
          setProgress(100);
        }
      });

      setStatus('ready');

      // Clear the pending update after successful download
      pendingUpdateRef.current = null;

      getVersion().then(currentVersion => {
        trackEvent("update_completed", {
          from_version: currentVersion,
          to_version: update.version,
        });
      }).catch(() => {});
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error('[Updater] Download failed:', msg);
      setError(msg);
      setStatus('error');
    }
  }, []);

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
