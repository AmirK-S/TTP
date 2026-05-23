// TTP - Talk To Paste
// Main entry point - handles routing for different windows
//
// Performance notes (v2.1.x polish pass 3):
//   - Window components are lazy-loaded so each window only downloads its own
//     chunk; the pill never pulls in Settings's ~70KB.
//   - `@sentry/react` is dynamically imported inside `initSentryIfConsented`,
//     not statically here — keeps it off the pill bundle entirely.
//   - i18n is initialised synchronously with a 'system'-resolved locale so
//     first paint never waits on the `get_settings` IPC round-trip; we patch
//     the persisted choice in afterwards if it differs.

import React, { Suspense, lazy } from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ErrorBoundary } from './lib/ErrorBoundary';
import { captureExceptionIfActive, initSentryIfConsented } from './lib/sentry';
import { initI18n, setLanguage, resolveLanguage, type LanguageChoice } from './i18n/config';
import './index.css';

// Lazy chunks: each window only fetches its own JS. The pill in particular
// stays tiny — no Settings, no Onboarding, no ApiKeySetup baggage.
const App = lazy(() => import('./App'));
const FloatingBar = lazy(() => import('./windows/FloatingBar'));
const ApiKeySetup = lazy(() => import('./windows/ApiKeySetup'));
const Onboarding = lazy(() => import('./windows/Onboarding'));
const Settings = lazy(() => import('./windows/Settings'));

/** Bridge our in-house ErrorBoundary to Sentry when the SDK is already active. */
function onBoundaryError(error: Error) {
  void captureExceptionIfActive(error);
}

/**
 * Get the current window and render the appropriate component.
 * - floating-bar: Renders the transparent recording indicator
 * - onboarding: Renders the first-launch permission onboarding
 * - setup: Renders the first-run API key setup window
 * - main (or others): Renders the main App component (hidden for tray app)
 */
function main() {
  // `?preview=onboarding|settings|pill|setup` is a dev-only override that
  // lets us inspect a window's UI in a plain browser (vite dev, screenshots,
  // visual diffs). When set, we skip every Tauri IPC + window probe — those
  // throw in a non-Tauri runtime and would crash the bootstrap. Production
  // builds never carry this query string.
  const previewLabel = new URLSearchParams(window.location.search).get('preview');
  const isPreview = previewLabel !== null;

  // Initialise i18n synchronously with the system-resolved locale so first
  // paint never blocks on IPC. The persisted choice (if it differs) is
  // patched in below — but the pill almost never shows translated text on
  // the very first frame anyway, so the visual difference is nil.
  const systemLang = resolveLanguage('system');
  initI18n(systemLang);

  if (!isPreview) {
    // Reconcile with the persisted language choice in the background.
    invoke<{ language?: string | null }>('get_settings')
      .then((s) => {
        const stored = (s?.language ?? 'system') as LanguageChoice;
        const resolved = resolveLanguage(stored);
        if (resolved !== systemLang) {
          setLanguage(stored);
        }
      })
      .catch(() => {
        // get_settings may fail on very first launch — defaults stay in place.
      });

    // Cross-window language sync: when any window saves a new language choice
    // via settings-store, every other window picks it up and re-renders.
    listen<{ language?: string | null }>('settings-changed', (event) => {
      setLanguage((event.payload?.language ?? 'system') as LanguageChoice);
    }).catch(() => {});

    // Fire-and-forget: gates itself on user telemetry consent (queried via
    // get_settings IPC); never throws, never blocks rendering. Dynamically
    // imports @sentry/react so the SDK stays out of the static bundle.
    void initSentryIfConsented();
  }

  const windowLabel = isPreview ? previewLabel : getCurrentWebviewWindow().label;

  const rootElement = document.getElementById('root') as HTMLElement;

  // Tiny fallback so a render-time crash doesn't leave a fully blank
  // window. The Sentry SDK still captures the error if telemetry is on.
  const fallback = <div style={{ padding: 16, fontFamily: 'system-ui' }}>Something went wrong.</div>;

  // Suspense fallback is null: the OS window is already visible with the
  // token background / transparent chrome — a flash of loading UI would be
  // worse than the empty frame.
  const suspenseFallback = null;

  const renderWindow = (node: React.ReactNode) => {
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <ErrorBoundary fallback={fallback} onError={onBoundaryError}>
          <Suspense fallback={suspenseFallback}>{node}</Suspense>
        </ErrorBoundary>
      </React.StrictMode>,
    );
  };

  if (windowLabel === 'floating-bar' || windowLabel === 'pill') {
    // Floating bar / pill window - transparent recording indicator
    renderWindow(<FloatingBar />);
  } else if (windowLabel === 'onboarding') {
    // Onboarding window - first-launch permission setup
    renderWindow(<Onboarding />);
  } else if (windowLabel === 'setup') {
    // Setup window - first-run API key configuration
    renderWindow(<ApiKeySetup />);
  } else if (windowLabel === 'settings') {
    // Settings window - app configuration and dictionary management
    renderWindow(<Settings />);
  } else {
    // Main window or any other window (hidden for tray-only app)
    renderWindow(<App />);
  }
}

main();
