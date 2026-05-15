// TTP - Talk To Paste
// Main App component - handles mic recording control and auto-update checking

import { useEffect } from 'react';
import { safeInvoke } from './lib/safeInvoke';
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

  // Check if we should show the "What's New" popup after an update
  useEffect(() => {
    safeInvoke<[string, string] | null>('check_whats_new').then(async (result) => {
      if (result) {
        try {
          const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
          const settingsWindow = await WebviewWindow.getByLabel('settings');
          if (settingsWindow) {
            await settingsWindow.show();
            await settingsWindow.setFocus();
          }
        } catch (e) {
          console.error('[WhatsNew] Failed to open settings window:', e);
        }
      }
    }).catch((err) => {
      console.error('[WhatsNew] Failed to check:', err);
    });
  }, []);

  // Auto-check + silent auto-install on launch and every 4 hours.
  // The previous flow popped open the Settings window every time an update
  // was found, but TTP is a menu-bar app — most users never open the main
  // window, so they never saw the update banner. The new flow downloads +
  // installs silently in the background, then the Rust tray displays a
  // blue-dot icon overlay + "Install update (vX.Y.Z)" menu item visible
  // in the menu bar at all times. No more Settings window popups.
  useUpdater({ autoCheck: true, autoInstall: true });

  // Window is hidden - TTP runs from the system tray
  return null;
}

export default App;
