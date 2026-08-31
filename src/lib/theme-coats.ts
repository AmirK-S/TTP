// TTP - Talk To Paste
// Coats — the app's aesthetic identity, and the seam the Companion's face
// reads its colours through.
//
// A coat is a palette, a type stack, a corner radius, a density, a surface
// treatment and a motion character, all changed together. It is orthogonal to
// light/dark: light and dark decide how the room is lit, the coat decides what
// the animal looks like in it. Every coat ships both appearances.
//
// ── What a coat may never do ────────────────────────────────────────────────
// Change what TTP does. There is nothing in this file, and nothing reachable
// from it, that the dictation path can observe. A locked install and an
// unlocked install take an identical route through a transcription; the only
// difference is which hex values the CSS resolves. If that ever stops being
// true this has become a paywall again — see docs/ttp-pro-design.md.
//
// ── Storage ─────────────────────────────────────────────────────────────────
// localStorage('ttp-coat'), mirrored into the inline bootstrap in index.html
// so the first paint of every window is already wearing the right coat.
//
// Deliberately NOT a field on the Rust `Settings` struct. `set_settings` takes
// a fixed shape and round-trips it through serde; a key the struct does not
// know about is dropped on the way in, so persisting there would silently lose
// the choice on the next launch. The existing light/dark preference has the
// same localStorage mirror for the same anti-flash reason — this rides along
// with it. If a coat field is ever added to the Rust settings, `readCoat` and
// `writeCoat` are the two functions that change.
//
// ── Cross-window ────────────────────────────────────────────────────────────
// Every window is its own webview and its own JS context. A coat picked in
// Settings has to reach the pill, so the change is broadcast on a Tauri event
// exactly like `settings-changed` and every window applies it locally.

/** A selectable coat. `free` is the one everybody has without buying anything. */
export interface Coat {
  id: string;
  free: boolean;
}

/**
 * The coat that paints when nothing has been chosen, when the chosen id is
 * unknown, or when cosmetics are locked. Never absent from `COATS`.
 *
 * `wild` is the biological sense of the word: the form the animal arrives in,
 * before anyone bred anything into it. It is the free coat and it is the one
 * built with the most care, because it is what almost everybody looks at.
 */
export const DEFAULT_COAT_ID = 'wild';

/**
 * The catalogue. Ids are persisted and must stay stable — renaming one
 * silently resets somebody's choice back to the default.
 *
 * Names and descriptions live in the frontend's locale files keyed by id, not
 * here, for the same reason the Rust sound packs stopped carrying English
 * strings: a bilingual app cannot keep user-facing prose next to the data.
 *
 * The four non-free coats unlock together, as one set, with one purchase.
 * There is no drip feed and there never will be: the moment a set is
 * incomplete by design and completion costs money, the delight is a compulsion
 * loop (fun-purchase-research.md, M6).
 */
export const COATS: readonly Coat[] = [
  { id: DEFAULT_COAT_ID, free: true },
  { id: 'roan', free: false },
  { id: 'piebald', free: false },
  { id: 'merle', free: false },
  { id: 'tortie', free: false },
];

const STORAGE_KEY = 'ttp-coat';
const DOM_ATTR = 'data-ttp-theme';
const SHIFT_CLASS = 'ttp-coat-shifting';
const SHIFT_MS = 460;
/** Broadcast name. Mirrors the `settings-changed` convention. */
export const COAT_EVENT = 'ttp-coat-changed';

/** Whether `id` names a coat we ship. */
export function isKnownCoat(id: string | null | undefined): boolean {
  return COATS.some((c) => c.id === id);
}

/**
 * Resolve the coat that should actually paint.
 *
 * Falls back to the default whenever the requested coat is unknown or is
 * locked. Never throws, never reports. A stale id from an older build or a
 * hand-edited file simply wears the house coat — the same silent, total
 * degradation `cosmetics::effective_sound_pack` gives sounds. A cosmetic that
 * nags about its own licence is worse than no cosmetic.
 */
export function effectiveCoat(requested: string | null | undefined, unlocked: boolean): string {
  if (!requested) return DEFAULT_COAT_ID;
  const coat = COATS.find((c) => c.id === requested);
  if (!coat) return DEFAULT_COAT_ID;
  return coat.free || unlocked ? coat.id : DEFAULT_COAT_ID;
}

/** The stored choice, unvalidated against the licence. */
export function readCoat(): string {
  try {
    if (typeof localStorage === 'undefined') return DEFAULT_COAT_ID;
    const raw = localStorage.getItem(STORAGE_KEY);
    return isKnownCoat(raw) ? (raw as string) : DEFAULT_COAT_ID;
  } catch {
    return DEFAULT_COAT_ID;
  }
}

function writeCoat(id: string): void {
  try {
    if (typeof localStorage === 'undefined') return;
    if (id === DEFAULT_COAT_ID) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // Some webview configurations disable localStorage. Worst case the coat
    // resets to `wild` on the next launch, which is a valid app.
  }
}

let shiftTimer: ReturnType<typeof setTimeout> | undefined;

/**
 * Paint a coat.
 *
 * `animate` turns on the one piece of theatre in the feature: for 460 ms every
 * colour property in the window interpolates, so the app turns into the new
 * coat instead of being replaced by it. It is a single class and a single
 * timer — no RAF, no per-frame JS, nothing left running afterwards — and it
 * transitions colour only, so nothing reflows and nothing moves under the
 * pointer. `prefers-reduced-motion` collapses it to an instant swap in CSS.
 */
export function applyCoat(id: string, options: { animate?: boolean } = {}): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const next = isKnownCoat(id) ? id : DEFAULT_COAT_ID;
  if (root.getAttribute(DOM_ATTR) === next) return;

  if (options.animate) {
    root.classList.add(SHIFT_CLASS);
    if (shiftTimer) clearTimeout(shiftTimer);
    shiftTimer = setTimeout(() => {
      root.classList.remove(SHIFT_CLASS);
      shiftTimer = undefined;
    }, SHIFT_MS);
  }

  root.setAttribute(DOM_ATTR, next);
}

/* ---------------------------------------------------------------------------
   The store.

   Small enough not to justify pulling the coat into the Zustand settings store
   — which is shared, saved as a whole object, and would drag a cosmetic choice
   into the same write path as the shortcut. useSyncExternalStore over the DOM
   attribute keeps the rendered picker and the painted document from being able
   to disagree.
   ------------------------------------------------------------------------ */

const listeners = new Set<() => void>();
let snapshot = DEFAULT_COAT_ID;

function publish(id: string): void {
  if (snapshot === id) return;
  snapshot = id;
  for (const fn of listeners) fn();
}

export function subscribeCoat(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function getCoatSnapshot(): string {
  return snapshot;
}

/**
 * Set the coat here and in every other open window, and remember it.
 *
 * `unlocked` is passed in rather than queried so this stays synchronous and
 * testable; the caller already knows, because it had to ask the backend to
 * render the picker at all. Selecting a locked coat resolves to the default
 * instead of erroring — the UI already disables those, so reaching this branch
 * means something odd happened and the right answer is still a working app.
 */
export function setCoat(id: string, unlocked: boolean, options: { animate?: boolean } = {}): string {
  const next = effectiveCoat(id, unlocked);
  writeCoat(next);
  applyCoat(next, options);
  publish(next);
  void broadcast(next);
  return next;
}

async function broadcast(id: string): Promise<void> {
  try {
    const { emit } = await import('@tauri-apps/api/event');
    await emit(COAT_EVENT, { coat: id });
  } catch {
    // Not running under Tauri (tests, `?preview=` in a plain browser), or the
    // event bridge is unavailable. The local window is already correct.
  }
}

/**
 * Bootstrap: paint the stored coat, then keep this window in step with the
 * others. Safe to call more than once; safe to call outside Tauri.
 *
 * The licence check is deliberately asynchronous and deliberately after the
 * first paint. A window that blocked on `cosmetics_unlocked` before painting
 * would make a cosmetic sit on the critical path of showing the UI, and if the
 * check ever hung, a coat would be able to stop TTP from appearing. Instead
 * the stored coat paints immediately and is quietly corrected to `wild` a tick
 * later if it turns out not to be owned.
 */
export function installCoat(): void {
  if (installed) return;
  installed = true;

  const stored = readCoat();
  applyCoat(stored);
  publish(stored);

  void (async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const unlocked = await invoke<boolean>('cosmetics_unlocked');
      const resolved = effectiveCoat(stored, Boolean(unlocked));
      if (resolved !== stored) {
        applyCoat(resolved);
        publish(resolved);
      }
    } catch {
      // No backend to ask. Leave the stored coat alone rather than yanking
      // the user's window back to `wild` because an IPC call failed.
    }
  })();

  void (async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      await listen<{ coat?: string }>(COAT_EVENT, (event) => {
        const id = event.payload?.coat;
        if (!isKnownCoat(id)) return;
        applyCoat(id as string, { animate: true });
        publish(id as string);
      });
    } catch {
      // Same window only. Nothing breaks; the pill just keeps its coat until
      // the next launch.
    }
  })();
}

let installed = false;

/** Test seam. Resets module state so each case starts from a clean document. */
export function __resetCoatsForTest(): void {
  installed = false;
  snapshot = DEFAULT_COAT_ID;
  listeners.clear();
  if (shiftTimer) {
    clearTimeout(shiftTimer);
    shiftTimer = undefined;
  }
}
