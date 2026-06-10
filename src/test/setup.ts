// Vitest setup — runs once per worker before any test file.
//
// What we do here:
//   1. Wire @testing-library/jest-dom so `expect(el).toBeInTheDocument()` etc.
//      live on the global `expect`.
//   2. Mock `@tauri-apps/api/core::invoke` so tests don't need a live Tauri
//      runtime. Individual tests override `mockInvoke` to script per-call
//      behaviour.
//   3. Mock `@tauri-apps/api/event::{listen, emit}` since the React surfaces
//      we test subscribe to those.

import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// Fake the Tauri bridge so `safeInvoke` doesn't sit in its 5s poll loop
// waiting for `window.__TAURI_INTERNALS__`. Production code never sees this
// at test time; the mock below intercepts the actual `invoke` call.
(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
if (typeof window !== 'undefined') {
  (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
}

// Module-scoped mock that individual tests can re-script.
export const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// Reset mocks between tests so leftover behaviour from one test can't bleed
// into the next.
import { afterEach } from 'vitest';
afterEach(() => {
  mockInvoke.mockReset();
});
