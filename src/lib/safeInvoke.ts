// TTP - Talk To Paste
// Safe wrapper around Tauri's invoke() that waits for the IPC bridge to be ready.
//
// Background: at cold start there is a brief window where the JS bundle has
// executed but window.__TAURI_INTERNALS__ has not been injected yet. Calling
// invoke() during this window throws "window.__TAURI_IPC__ is not a function"
// (the v1.6.x Sentry crash). This wrapper polls until the bridge is present,
// caches the readiness promise, then forwards to the real invoke.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

const POLL_INTERVAL_MS = 25;
const MAX_WAIT_MS = 5000;

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

let bridgeReady: Promise<void> | null = null;

function isBridgeReady(): boolean {
  return typeof window !== 'undefined' && typeof window.__TAURI_INTERNALS__ !== 'undefined';
}

function waitForBridge(): Promise<void> {
  if (bridgeReady) return bridgeReady;
  const promise = new Promise<void>((resolve, reject) => {
    if (isBridgeReady()) {
      resolve();
      return;
    }
    const start = Date.now();
    const timer = setInterval(() => {
      if (isBridgeReady()) {
        clearInterval(timer);
        resolve();
      } else if (Date.now() - start > MAX_WAIT_MS) {
        clearInterval(timer);
        reject(new Error(`Tauri IPC bridge not ready after ${MAX_WAIT_MS}ms`));
      }
    }, POLL_INTERVAL_MS);
  });
  bridgeReady = promise;
  // If this attempt rejects, drop the cached promise so the next safeInvoke
  // call gets a fresh wait. Otherwise a single early failure (slow webview
  // boot, transient OS hiccup) would stick a rejected promise in the cache
  // and break every subsequent invoke for the whole session.
  promise.catch(() => {
    if (bridgeReady === promise) bridgeReady = null;
  });
  return promise;
}

export async function safeInvoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  await waitForBridge();
  return tauriInvoke<T>(cmd, args);
}
