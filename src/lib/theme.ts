// TTP - Talk To Paste
// Theme controller — manages light / dark / system appearance.
//
// Storage model:
//   - Persisted in Settings.theme as 'system' | 'light' | 'dark' (Rust JSON).
//   - Mirrored to localStorage('ttp-theme') so the inline bootstrap script in
//     index.html can set <html data-theme> BEFORE any CSS paints. Without
//     that mirror, a user who forced "dark" while on a "light" macOS would
//     see a white flash on every window open.
//
// CSS contract (see index.css):
//   - Default `:root` carries light tokens.
//   - `@media (prefers-color-scheme: dark) :root:not([data-theme="light"])` swaps to dark.
//   - `:root[data-theme="dark"]` forces dark regardless of the OS preference.
//
// "system" mode removes the data-theme attribute so the media query rules.
// "light" / "dark" set the attribute explicitly to override the OS.
//
// We also keep a matchMedia listener active so that — when the user is in
// "system" mode — every macOS appearance flip is reflected immediately
// without a re-render (the CSS media query already covers it, but we still
// emit a custom event so React components that need to branch on the
// resolved theme can subscribe).

export type ThemeChoice = 'system' | 'light' | 'dark';

const STORAGE_KEY = 'ttp-theme';
const DOM_ATTR = 'data-theme';

let mediaListenerInstalled = false;

/** Read the OS-level dark-mode preference. Falls back to 'light' outside a browser. */
function osPrefersDark(): boolean {
  if (typeof window === 'undefined' || !window.matchMedia) return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

/** Resolve a stored choice to the concrete theme that will actually paint. */
export function resolveTheme(choice: ThemeChoice | null | undefined): 'light' | 'dark' {
  if (choice === 'light' || choice === 'dark') return choice;
  return osPrefersDark() ? 'dark' : 'light';
}

/** Persist mirror + apply the data-theme attribute that the CSS reads. */
export function applyTheme(choice: ThemeChoice | null | undefined): void {
  const normalised: ThemeChoice =
    choice === 'light' || choice === 'dark' || choice === 'system' ? choice : 'system';

  if (typeof document !== 'undefined') {
    const root = document.documentElement;
    if (normalised === 'system') {
      root.removeAttribute(DOM_ATTR);
    } else {
      root.setAttribute(DOM_ATTR, normalised);
    }
    // color-scheme hints the UA (scrollbars, form controls, default text-selection).
    root.style.colorScheme = resolveTheme(normalised);
  }

  try {
    if (typeof localStorage !== 'undefined') {
      if (normalised === 'system') {
        localStorage.removeItem(STORAGE_KEY);
      } else {
        localStorage.setItem(STORAGE_KEY, normalised);
      }
    }
  } catch {
    // Ignore — localStorage can be disabled in some webview configs; the
    // worst-case is a 1-frame flash on next launch.
  }
}

/** Install a one-time `prefers-color-scheme` listener so the `color-scheme`
 *  CSS property updates when the OS flips and the user is in "system" mode.
 *  CSS media-query rules update on their own; this listener exists only to
 *  keep the inline `style.color-scheme` we set in `applyTheme` in sync. */
export function installSystemThemeListener(getChoice: () => ThemeChoice): void {
  if (mediaListenerInstalled) return;
  if (typeof window === 'undefined' || !window.matchMedia) return;
  mediaListenerInstalled = true;

  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  const handler = () => {
    if (getChoice() === 'system') {
      if (typeof document !== 'undefined') {
        document.documentElement.style.colorScheme = osPrefersDark() ? 'dark' : 'light';
      }
    }
  };
  // Safari < 14 only supported addListener; modern WebKit (which Tauri uses)
  // supports addEventListener. Stick to the standard API.
  mq.addEventListener('change', handler);
}
