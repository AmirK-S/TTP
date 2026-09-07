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
import { applyTheme, installSystemThemeListener, type ThemeChoice } from './lib/theme';
import { installCoat } from './lib/theme-coats';
import { useSettingsStore } from './stores/settings-store';
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

  // Live theme tracker — the matchMedia listener in lib/theme.ts checks this
  // before re-applying `color-scheme` so it only acts when we're in "system"
  // mode. Mutated below by the settings reconciliation + cross-window sync.
  let currentTheme: ThemeChoice = 'system';
  installSystemThemeListener(() => currentTheme);

  // The coat: paint the stored one, then stay in step with the other windows.
  // Runs for preview builds too — a coat is pure CSS, so it is exactly as
  // valid in a plain browser as it is under Tauri, and every Tauri call inside
  // is guarded. Independent of `data-theme` above: light/dark and the coat are
  // orthogonal and neither reads the other.
  installCoat();

  if (!isPreview) {
    // Reconcile with the persisted language + theme choice in the background.
    // The anti-flash <script> in index.html already painted the right theme
    // from localStorage; this round-trip catches the case where the on-disk
    // setting was changed by another install (rare) or by `reset_settings`.
    invoke<{ language?: string | null; theme?: string | null }>('get_settings')
      .then((s) => {
        const stored = (s?.language ?? 'system') as LanguageChoice;
        const resolved = resolveLanguage(stored);
        if (resolved !== systemLang) {
          setLanguage(stored);
        }
        const storedTheme = (s?.theme ?? 'system') as ThemeChoice;
        currentTheme = storedTheme;
        applyTheme(storedTheme);
      })
      .catch(() => {
        // get_settings may fail on very first launch — defaults stay in place.
      });

    // Cross-window settings sync: every window has its own Zustand instance
    // (separate webview = separate JS context = separate store), so when one
    // window saves a setting, the others would stay stale until their own
    // saveSettings stamped its snapshot back over disk — overwriting the
    // first window's change. We listen for the `settings-changed` event the
    // Rust `set_settings` command emits and mirror every field into this
    // window's store so the next save merges on top of the right baseline.
    listen<{
      ai_polish_enabled?: boolean;
      fn_key_enabled?: boolean;
      telemetry_enabled?: boolean;
      hands_free_mode?: boolean;
      hide_pill_when_inactive?: boolean;
      autostart_enabled?: boolean;
      history_enabled?: boolean;
      use_beta_channel?: boolean;
      shortcut?: string;
      language?: string | null;
      theme?: string | null;
    }>('settings-changed', (event) => {
      const p = event.payload;
      setLanguage((p?.language ?? 'system') as LanguageChoice);
      const nextTheme = (p?.theme ?? 'system') as ThemeChoice;
      currentTheme = nextTheme;
      applyTheme(nextTheme);
      // Mirror the full payload into the Zustand store so toggles UI in
      // every open window reflects reality immediately.
      useSettingsStore.setState({
        ...(p?.ai_polish_enabled !== undefined && { aiPolishEnabled: p.ai_polish_enabled }),
        ...(p?.fn_key_enabled !== undefined && { fnKeyEnabled: p.fn_key_enabled }),
        ...(p?.telemetry_enabled !== undefined && { telemetryEnabled: p.telemetry_enabled }),
        ...(p?.hands_free_mode !== undefined && { handsFreeMode: p.hands_free_mode }),
        ...(p?.hide_pill_when_inactive !== undefined && { hidePillWhenInactive: p.hide_pill_when_inactive }),
        ...(p?.autostart_enabled !== undefined && { autostartEnabled: p.autostart_enabled }),
        ...(p?.history_enabled !== undefined && { historyEnabled: p.history_enabled }),
        ...(p?.use_beta_channel !== undefined && { useBetaChannel: p.use_beta_channel }),
        ...(p?.shortcut !== undefined && { shortcut: p.shortcut }),
        ...(p?.language !== undefined && { language: (p.language ?? 'system') as LanguageChoice }),
        ...(p?.theme !== undefined && { theme: nextTheme }),
      });
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
