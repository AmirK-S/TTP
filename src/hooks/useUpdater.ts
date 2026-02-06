// TTP - Talk To Paste
// Hook for checking and installing app updates

import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useState, useCallback } from 'react';

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error';

export interface UpdateInfo {
  version: string;
  body?: string;
}

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [progress, setProgress] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);

  const checkForUpdates = useCallback(async () => {
    setStatus('checking');
    setError(null);

    try {
      const update = await check();

      if (update?.available) {
        setUpdateInfo({
          version: update.version,
          body: update.body,
        });
        setStatus('available');
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

  const downloadAndInstall = useCallback(async () => {
    setStatus('downloading');
    setProgress(0);

    try {
      const update = await check();

      if (!update?.available) {
        setStatus('idle');
        return;
      }

      await update.downloadAndInstall((event) => {
        if (event.event === 'Started' && event.data.contentLength) {
          setProgress(0);
        } else if (event.event === 'Progress') {
          // Calculate progress percentage
          setProgress((prev) => Math.min(prev + (event.data.chunkLength / 1000000) * 10, 99));
        } else if (event.event === 'Finished') {
          setProgress(100);
        }
      });

      setStatus('ready');
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

  return {
    status,
    updateInfo,
    progress,
    error,
    checkForUpdates,
    downloadAndInstall,
    restartApp,
  };
}
