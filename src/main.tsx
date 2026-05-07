// TTP - Talk To Paste
// Main entry point - handles routing for different windows

import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import App from './App';
import FloatingBar from './windows/FloatingBar';
import ApiKeySetup from './windows/ApiKeySetup';
import Onboarding from './windows/Onboarding';
import Settings from './windows/Settings';
import { ErrorBoundary, initSentryIfConsented } from './lib/sentry';
import './index.css';

/**
 * Get the current window and render the appropriate component.
 * - floating-bar: Renders the transparent recording indicator
 * - onboarding: Renders the first-launch permission onboarding
 * - setup: Renders the first-run API key setup window
 * - main (or others): Renders the main App component (hidden for tray app)
 */
async function main() {
  // Fire-and-forget: gates itself on user telemetry consent (queried via
  // get_settings IPC); never throws, never blocks rendering.
  void initSentryIfConsented();

  const currentWindow = getCurrentWebviewWindow();
  const windowLabel = currentWindow.label;

  const rootElement = document.getElementById('root') as HTMLElement;

  // Tiny fallback so a render-time crash doesn't leave a fully blank
  // window. The Sentry SDK still captures the error if telemetry is on.
  const fallback = <div style={{ padding: 16, fontFamily: 'system-ui' }}>Something went wrong.</div>;

  if (windowLabel === 'floating-bar' || windowLabel === 'pill') {
    // Floating bar / pill window - transparent recording indicator
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary fallback={fallback}>
          <FloatingBar />
        </ErrorBoundary>
      </React.StrictMode>
    );
  } else if (windowLabel === 'onboarding') {
    // Onboarding window - first-launch permission setup
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary fallback={fallback}>
          <Onboarding />
        </ErrorBoundary>
      </React.StrictMode>
    );
  } else if (windowLabel === 'setup') {
    // Setup window - first-run API key configuration
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary fallback={fallback}>
          <ApiKeySetup />
        </ErrorBoundary>
      </React.StrictMode>
    );
  } else if (windowLabel === 'settings') {
    // Settings window - app configuration and dictionary management
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary fallback={fallback}>
          <Settings />
        </ErrorBoundary>
      </React.StrictMode>
    );
  } else {
    // Main window or any other window (hidden for tray-only app)
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary fallback={fallback}>
          <App />
        </ErrorBoundary>
      </React.StrictMode>
    );
  }
}

main();
