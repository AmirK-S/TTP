import { describe, it, expect, beforeEach } from 'vitest';
import { applyTheme, resolveTheme } from './theme';

// happy-dom doesn't always wire `localStorage` onto the global by default
// when modules are loaded in vitest mode. Install a minimal Storage shim
// so the theme module's persistence path is exercised under test. Real
// browsers and the Tauri webview always provide a real localStorage.
const store = new Map<string, string>();
const fakeStorage = {
  getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
  setItem: (k: string, v: string) => store.set(k, v),
  removeItem: (k: string) => {
    store.delete(k);
  },
  clear: () => store.clear(),
  key: (i: number) => Array.from(store.keys())[i] ?? null,
  get length() {
    return store.size;
  },
} as Storage;
Object.defineProperty(globalThis, 'localStorage', {
  configurable: true,
  value: fakeStorage,
});

// Each test resets the document state so the previous test's data-theme
// attribute can't leak into the next. happy-dom provides a live document.
beforeEach(() => {
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.style.colorScheme = '';
  store.clear();
});

describe('resolveTheme', () => {
  it('returns the explicit choice when light or dark', () => {
    expect(resolveTheme('light')).toBe('light');
    expect(resolveTheme('dark')).toBe('dark');
  });

  it('falls back to OS preference when system', () => {
    const result = resolveTheme('system');
    expect(['light', 'dark']).toContain(result);
  });

  it('treats null and undefined as system', () => {
    expect(['light', 'dark']).toContain(resolveTheme(null));
    expect(['light', 'dark']).toContain(resolveTheme(undefined));
  });
});

describe('applyTheme', () => {
  it('sets data-theme="dark" for explicit dark choice', () => {
    applyTheme('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('sets data-theme="light" for explicit light choice', () => {
    applyTheme('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('removes data-theme for system choice (so OS media query rules)', () => {
    // Set to dark first so we can verify it gets removed.
    applyTheme('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');

    applyTheme('system');
    expect(document.documentElement.getAttribute('data-theme')).toBeNull();
  });

  it('mirrors explicit choices to localStorage for the inline bootstrap script', () => {
    applyTheme('dark');
    expect(localStorage.getItem('ttp-theme')).toBe('dark');

    applyTheme('light');
    expect(localStorage.getItem('ttp-theme')).toBe('light');
  });

  it('clears localStorage on system so the bootstrap script reads OS prefs', () => {
    applyTheme('dark');
    expect(localStorage.getItem('ttp-theme')).toBe('dark');

    applyTheme('system');
    expect(localStorage.getItem('ttp-theme')).toBeNull();
  });

  it('treats unknown values as system (defensive fallback)', () => {
    applyTheme('dark');
    applyTheme('foo' as unknown as 'system');
    expect(document.documentElement.getAttribute('data-theme')).toBeNull();
  });

  it('treats null/undefined as system', () => {
    applyTheme('dark');
    applyTheme(null);
    expect(document.documentElement.getAttribute('data-theme')).toBeNull();

    applyTheme('light');
    applyTheme(undefined);
    expect(document.documentElement.getAttribute('data-theme')).toBeNull();
  });
});
