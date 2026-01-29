// TTP - Talk To Paste
// Main entry point - handles routing for different windows

import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import App from './App';
import FloatingBar from './windows/FloatingBar';
import './index.css';

/**
 * Get the current window and render the appropriate component.
 * - floating-bar: Renders the transparent recording indicator
 * - main (or others): Renders the main App component
 */
async function main() {
  const currentWindow = getCurrentWebviewWindow();
  const windowLabel = currentWindow.label;

  const rootElement = document.getElementById('root') as HTMLElement;

  if (windowLabel === 'floating-bar') {
    // Floating bar window - transparent recording indicator
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <FloatingBar />
      </React.StrictMode>
    );
  } else {
    // Main window or any other window
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>
    );
  }
}

main();
