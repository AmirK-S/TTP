// TTP - Talk To Paste
// Main App component - handles mic recording control

import { useRecordingControl } from './hooks/useRecordingControl';

/**
 * Main App component - this window is hidden by default.
 * TTP runs from the system tray, but this component handles
 * the mic recording control by listening to backend state changes.
 */
function App() {
  // This hook listens for recording state changes from the backend
  // and controls the actual microphone recording via tauri-plugin-mic-recorder
  useRecordingControl({
    onRecordingComplete: (result) => {
      console.log('[App] Recording complete:', result.filePath);
    },
    onError: (error) => {
      console.error('[App] Recording error:', error);
    },
  });

  // Window is hidden - TTP runs from the system tray
  return null;
}

export default App;
