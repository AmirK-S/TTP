// TTP - Talk To Paste
// Appearance follows macOS. There is no light/dark setting.
//
// CSS contract (see src/styles/coats.css):
//   - Default `:root` carries light tokens.
//   - `@media (prefers-color-scheme: dark)` swaps to dark.
//
// The media query does the painting on its own. This module only keeps the
// inline `color-scheme` property (scrollbars, form controls) in step with the
// OS, and clears what an older build's explicit light/dark choice left behind.

const LEGACY_STORAGE_KEY = 'ttp-theme';

function osPrefersDark(): boolean {
  if (typeof window === 'undefined' || !window.matchMedia) return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function syncColorScheme(): void {
  if (typeof document === 'undefined') return;
  document.documentElement.style.colorScheme = osPrefersDark() ? 'dark' : 'light';
}

let installed = false;

/** Follow the OS appearance for this window. Idempotent. */
export function followSystemTheme(): void {
  if (typeof document !== 'undefined') {
    document.documentElement.removeAttribute('data-theme');
  }
  try {
    if (typeof localStorage !== 'undefined') localStorage.removeItem(LEGACY_STORAGE_KEY);
  } catch {
    // localStorage can be disabled in some webview configs; nothing reads it.
  }
  syncColorScheme();

  if (installed || typeof window === 'undefined' || !window.matchMedia) return;
  installed = true;
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', syncColorScheme);
}
