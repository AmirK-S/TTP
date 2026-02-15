// TTP - Talk To Paste
// Main App component - handles mic recording control and auto-update checking

import { useEffect } from 'react';
import { useRecordingControl } from './hooks/useRecordingControl';
import { useUpdater } from './hooks/useUpdater';

/**
 * Main App component - this window is hidden by default.
 * TTP runs from the system tray, but this component handles
 * the mic recording control by listening to backend state changes
 * and checks for updates automatically on launch + every 4 hours.
 */
function App() {
  // This hook listens for recording state changes from the backend
  // and controls the actual microphone recording via tauri-plugin-mic-recorder
  useRecordingControl({
    onRecordingComplete: () => {},
    onError: () => {},
  });

  // Auto-check for updates on launch and every 4 hours
  // shouldNotify becomes true when update is available AND app is idle
  const { shouldNotify, updateInfo } = useUpdater({ autoCheck: true });

  // When an update is found and the app is idle, open Settings and emit event
  useEffect(() => {
    if (shouldNotify && updateInfo) {
      (async () => {
        try {
          const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
          const { emit } = await import('@tauri-apps/api/event');
          const settingsWindow = await WebviewWindow.getByLabel('settings');
          if (settingsWindow) {
            await settingsWindow.show();
            await settingsWindow.setFocus();
          }
          await emit('update-available', { version: updateInfo.version, body: updateInfo.body });
        } catch (e) {
          console.error('[Updater] Failed to show update notification:', e);
        }
      })();
    }
  }, [shouldNotify, updateInfo]);

  // Window is hidden - TTP runs from the system tray
  return null;
}

export default App;
