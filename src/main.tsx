// TTP - Talk To Paste
// Main entry point - handles routing for different windows

import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App';
import FloatingBar from './windows/FloatingBar';
import ApiKeySetup from './windows/ApiKeySetup';
import Onboarding from './windows/Onboarding';
import Settings from './windows/Settings';
import { ErrorBoundary, initSentryIfConsented } from './lib/sentry';
import { initI18n, setLanguage, type LanguageChoice } from './i18n/config';
import './index.css';

/**
 * Get the current window and render the appropriate component.
 * - floating-bar: Renders the transparent recording indicator
 * - onboarding: Renders the first-launch permission onboarding
 * - setup: Renders the first-run API key setup window
 * - main (or others): Renders the main App component (hidden for tray app)
 */
async function main() {
  // `?preview=onboarding|settings|pill|setup` is a dev-only override that
  // lets us inspect a window's UI in a plain browser (vite dev, screenshots,
  // visual diffs). When set, we skip every Tauri IPC + window probe — those
  // throw in a non-Tauri runtime and would crash the bootstrap. Production
  // builds never carry this query string.
  const previewLabel = new URLSearchParams(window.location.search).get('preview');
  const isPreview = previewLabel !== null;

  if (!isPreview) {
    // Fire-and-forget: gates itself on user telemetry consent (queried via
    // get_settings IPC); never throws, never blocks rendering.
    void initSentryIfConsented();
  }

  // Initialize i18n synchronously with the persisted language choice (or
  // 'system' if first launch). We read settings once here to avoid a render
  // flash; further changes propagate via the 'settings-changed' event below.
  let initialLang: LanguageChoice = 'system';
  if (!isPreview) {
    try {
      const s = await invoke<{ language?: string | null }>('get_settings');
      initialLang = ((s?.language ?? 'system') as LanguageChoice);
    } catch {
      // get_settings may fail on very first launch — defaults to 'system'.
    }
  }
  initI18n(initialLang);

  if (!isPreview) {
    // Cross-window language sync: when any window saves a new language choice
    // via settings-store, every other window picks it up and re-renders.
    listen<{ language?: string | null }>('settings-changed', (event) => {
      setLanguage((event.payload?.language ?? 'system') as LanguageChoice);
    }).catch(() => {});
  }

  const windowLabel = isPreview ? previewLabel : getCurrentWebviewWindow().label;

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
