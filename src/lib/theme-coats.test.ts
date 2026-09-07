// TTP - Talk To Paste
// Coats: the catalogue's invariants, the licence fallback, and the contract
// the Companion's face codes against.
//
// The interesting tests here are the last two. `effectiveCoat` is small enough
// to read, but "every coat has a stylesheet block" and "every coat has copy in
// both locales" are the two things that will actually break when somebody adds
// a sixth coat six months from now, and neither has any other guard.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { mockInvoke } from '../test/setup';
import {
  COATS,
  COAT_EVENT,
  DEFAULT_COAT_ID,
  applyCoat,
  effectiveCoat,
  getCoatSnapshot,
  installCoat,
  isKnownCoat,
  readCoat,
  setCoat,
  subscribeCoat,
  __resetCoatsForTest,
} from './theme-coats';

const read = (rel: string) =>
  readFileSync(fileURLToPath(new URL(rel, import.meta.url)), 'utf-8');

// happy-dom does not alias `window.localStorage` onto the global scope, and
// the module under test reads the bare global (which is what a WKWebView
// provides). Its own `typeof localStorage === 'undefined'` guard means the
// absence is handled rather than thrown on — but then none of the persistence
// tests would exercise anything, so give the tests a real Storage to watch.
const store = new Map<string, string>();
const shim: Storage = {
  get length() { return store.size; },
  clear: () => store.clear(),
  getItem: (k) => (store.has(k) ? store.get(k)! : null),
  key: (i) => [...store.keys()][i] ?? null,
  removeItem: (k) => { store.delete(k); },
  setItem: (k, v) => { store.set(k, String(v)); },
};
Object.defineProperty(globalThis, 'localStorage', { value: shim, configurable: true });

beforeEach(async () => {
  // Drain anything the previous test left in flight. `persistCoat` reaches
  // `invoke` through a dynamic import, so its call can land a macrotask after
  // the test that caused it returned — and would then be counted against the
  // next one.
  await new Promise((r) => setTimeout(r, 0));
  mockInvoke.mockReset();
  __resetCoatsForTest();
  localStorage.clear();
  document.documentElement.removeAttribute('data-ttp-theme');
  document.documentElement.className = '';
});

describe('the catalogue', () => {
  it('has a default that is free', () => {
    const d = COATS.find((c) => c.id === DEFAULT_COAT_ID);
    expect(d).toBeDefined();
    expect(d?.free).toBe(true);
  });

  it('has exactly one free coat', () => {
    // Two free coats would make "the free default" ambiguous, and the picker
    // gives the free one the full-width tile.
    expect(COATS.filter((c) => c.free)).toHaveLength(1);
  });

  it('has unique ids', () => {
    const ids = COATS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('has ids that are safe as translation keys and CSS attribute values', () => {
    for (const c of COATS) {
      expect(c.id).toMatch(/^[a-z][a-z_]*$/);
    }
  });
});

describe('effectiveCoat', () => {
  it('falls back to the default for an unknown id', () => {
    // A stale id from an older build, or a hand-edited localStorage value.
    expect(effectiveCoat('no-such-coat', true)).toBe(DEFAULT_COAT_ID);
  });

  it('falls back to the default when nothing is chosen', () => {
    expect(effectiveCoat(null, true)).toBe(DEFAULT_COAT_ID);
    expect(effectiveCoat(undefined, false)).toBe(DEFAULT_COAT_ID);
  });

  it('refuses a locked coat, silently', () => {
    const locked = COATS.find((c) => !c.free)!;
    expect(effectiveCoat(locked.id, false)).toBe(DEFAULT_COAT_ID);
  });

  it('allows a locked coat once unlocked', () => {
    const locked = COATS.find((c) => !c.free)!;
    expect(effectiveCoat(locked.id, true)).toBe(locked.id);
  });

  it('keeps the free coat available whatever the licence says', () => {
    // The silent-degradation guarantee: there is no state in which a licence
    // check leaves TTP without a working appearance.
    expect(effectiveCoat(DEFAULT_COAT_ID, false)).toBe(DEFAULT_COAT_ID);
  });
});

describe('applying and persisting', () => {
  it('paints the coat onto the document element', () => {
    applyCoat('roan');
    expect(document.documentElement.getAttribute('data-ttp-theme')).toBe('roan');
  });

  it('paints the default rather than an unknown id', () => {
    applyCoat('nonsense');
    expect(document.documentElement.getAttribute('data-ttp-theme')).toBe(DEFAULT_COAT_ID);
  });

  it('stores a non-default choice and clears the key for the default', () => {
    setCoat('merle', true);
    expect(localStorage.getItem('ttp-coat')).toBe('merle');
    expect(readCoat()).toBe('merle');

    setCoat(DEFAULT_COAT_ID, true);
    // Absence means "the default", so the key is removed rather than written —
    // the anti-flash script in index.html reads it and must not have to know
    // the default's name.
    expect(localStorage.getItem('ttp-coat')).toBeNull();
    expect(readCoat()).toBe(DEFAULT_COAT_ID);
  });

  it('never stores a locked coat', () => {
    setCoat('merle', false);
    expect(localStorage.getItem('ttp-coat')).toBeNull();
    expect(document.documentElement.getAttribute('data-ttp-theme')).toBe(DEFAULT_COAT_ID);
  });

  it('ignores a junk value already in storage', () => {
    localStorage.setItem('ttp-coat', 'gingham');
    expect(readCoat()).toBe(DEFAULT_COAT_ID);
    expect(isKnownCoat('gingham')).toBe(false);
  });

  it('notifies subscribers with the resolved id', () => {
    const seen: string[] = [];
    const stop = subscribeCoat(() => seen.push(getCoatSnapshot()));
    setCoat('tortie', true);
    setCoat('roan', false); // locked -> resolves to the default
    stop();
    expect(seen).toEqual(['tortie', DEFAULT_COAT_ID]);
  });
});

describe('persisting to the settings store', () => {
  // Debt item 1 in docs/overhaul-status.md. The coat used to live in
  // `localStorage` and nowhere else, because `set_settings` round-tripped a
  // fixed serde shape and dropped anything it did not recognise. That is now
  // fixed on the Rust side (`settings::store::merge_payload`), so the coat can
  // be what it should always have been: a real setting, with `localStorage`
  // demoted to the pre-paint mirror the anti-flash script in index.html reads.

  it('sends the chosen coat to the settings store, not only to localStorage', async () => {
    setCoat('merle', true);
    await vi.waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_settings', { settings: { coat: 'merle' } }),
    );
  });

  it('sends a partial payload and relies on the backend merge', async () => {
    // The whole payload would need every other setting, which this module has
    // no business knowing — and sending a stale copy of them is how v2.1.2
    // corrupted settings.json. `merge_payload` on the Rust side keeps every
    // field this object does not mention.
    setCoat('roan', true);
    await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    const [, args] = mockInvoke.mock.calls.at(-1)!;
    expect(Object.keys((args as { settings: object }).settings)).toEqual(['coat']);
  });

  it('clears the stored coat rather than writing the default id', async () => {
    // `null` and "wild" mean the same thing to the backend; `null` is the one
    // that lets the default be renamed without migrating anybody's file.
    setCoat(DEFAULT_COAT_ID, true);
    await vi.waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_settings', { settings: { coat: null } }),
    );
  });

  it('never persists a locked coat', async () => {
    setCoat('merle', false);
    await vi.waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_settings', { settings: { coat: null } }),
    );
  });
});

describe('booting a window', () => {
  // `installCoat` runs in every window, before React. The invariants:
  // paint immediately from the mirror, then let the backend correct it.

  const called = (cmd: string) => mockInvoke.mock.calls.some((c) => c[0] === cmd);
  const flush = () => vi.waitFor(() => expect(called('cosmetics_unlocked')).toBe(true));

  const backend = (settings: unknown, unlocked: boolean) => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(settings);
      if (cmd === 'cosmetics_unlocked') return Promise.resolve(unlocked);
      return Promise.resolve(undefined);
    });
  };

  it('restores a coat that localStorage has never heard of', async () => {
    // The literal debt: the user cleared site data, or is on a fresh webview
    // profile. Before this, their coat was simply gone.
    backend({ coat: 'tortie' }, true);
    installCoat();
    expect(document.documentElement.getAttribute('data-ttp-theme')).toBe(DEFAULT_COAT_ID);
    await flush();
    await vi.waitFor(() => {
      expect(document.documentElement.getAttribute('data-ttp-theme')).toBe('tortie');
    });
    // and the mirror is repaired, so the NEXT window opens without a flash
    expect(localStorage.getItem('ttp-coat')).toBe('tortie');
    expect(getCoatSnapshot()).toBe('tortie');
  });

  it('lets the backend overrule a stale mirror', async () => {
    localStorage.setItem('ttp-coat', 'roan');
    backend({ coat: 'merle' }, true);
    installCoat();
    await flush();
    await vi.waitFor(() => {
      expect(document.documentElement.getAttribute('data-ttp-theme')).toBe('merle');
    });
    expect(localStorage.getItem('ttp-coat')).toBe('merle');
  });

  it('migrates a localStorage-only choice into the settings store, once', async () => {
    // Every user who picked a coat before it was a real setting. Their choice
    // exists only in the mirror; the first launch after this change moves it.
    localStorage.setItem('ttp-coat', 'piebald');
    backend({ coat: null }, true);
    installCoat();
    await flush();
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_settings', { settings: { coat: 'piebald' } });
    });
    expect(document.documentElement.getAttribute('data-ttp-theme')).toBe('piebald');
  });

  it('does not write anything back when there is nothing to migrate', async () => {
    backend({ coat: null }, true);
    installCoat();
    await flush();
    expect(mockInvoke).not.toHaveBeenCalledWith('set_settings', expect.anything());
  });

  it('paints the house coat when the stored one is not owned', async () => {
    localStorage.setItem('ttp-coat', 'merle');
    backend({ coat: 'merle' }, false);
    installCoat();
    await flush();
    await vi.waitFor(() => {
      expect(document.documentElement.getAttribute('data-ttp-theme')).toBe(DEFAULT_COAT_ID);
    });
    // The choice itself is NOT destroyed — a licence that lapses and comes
    // back must find the same coat waiting, not a reset one.
    expect(localStorage.getItem('ttp-coat')).toBe('merle');
    expect(mockInvoke).not.toHaveBeenCalledWith('set_settings', expect.anything());
  });

  it('keeps the mirror-painted coat when the backend cannot be reached', async () => {
    // A window that yanked itself back to `wild` because an IPC call failed
    // would be a cosmetic taking the app down with it.
    localStorage.setItem('ttp-coat', 'merle');
    mockInvoke.mockRejectedValue(new Error('no backend'));
    installCoat();
    await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(document.documentElement.getAttribute('data-ttp-theme')).toBe('merle');
    expect(localStorage.getItem('ttp-coat')).toBe('merle');
  });
});

describe('the coat change animation', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('adds the transition class and takes it off again', () => {
    // It has to come off. A permanent global colour transition would make
    // every hover in the app feel like it is wading through treacle.
    applyCoat('roan', { animate: true });
    expect(document.documentElement.classList.contains('ttp-coat-shifting')).toBe(true);
    vi.advanceTimersByTime(1000);
    expect(document.documentElement.classList.contains('ttp-coat-shifting')).toBe(false);
  });

  it('does not animate when the coat has not actually changed', () => {
    applyCoat('roan');
    applyCoat('roan', { animate: true });
    expect(document.documentElement.classList.contains('ttp-coat-shifting')).toBe(false);
  });
});

describe('the stylesheet contract', () => {
  const css = read('../styles/coats.css');

  it('declares a block for every coat', () => {
    for (const c of COATS) {
      // `wild` shares the base block, which names it explicitly so its tokens
      // also resolve on a swatch inside a document wearing another coat.
      expect(css).toContain(`[data-ttp-theme="${c.id}"]`);
    }
  });

  it('publishes the four names the Companion face reads', () => {
    // Renaming or dropping any of these silently breaks a component in
    // another file that this project cannot type-check across.
    for (const name of ['--ttp-accent', '--ttp-surface', '--ttp-ink', '--ttp-glow']) {
      expect(css).toContain(`${name}:`);
    }
  });

  it('resolves the pill-side meaning of surface and ink in the pill window', () => {
    // The face reads `--ttp-surface`/`--ttp-ink` without knowing which window
    // it is in, so the floating bar has to redefine them.
    const scoped = css.slice(css.indexOf('html.floating-bar'));
    expect(scoped).toContain('--ttp-surface:');
    expect(scoped).toContain('--ttp-ink:');
  });

  it('gives every coat a pill body and a mark that reads on it', () => {
    // Both are appearance-independent: the pill floats over wallpaper.
    expect(css.match(/--ttp-pill:/g) ?? []).toHaveLength(COATS.length);
    expect(css.match(/--ttp-pill-ink:/g) ?? []).toHaveLength(COATS.length);
  });

  it('honours reduced motion for the one thing that moves', () => {
    expect(css).toContain('prefers-reduced-motion');
  });

  it('gives every coat the full set of tokens, declared once each', () => {
    // The failure this catches is a half-added coat: a block that overrides
    // some of the palette and silently inherits the rest from `wild`, which
    // looks fine in one appearance and wrong in the other. It also catches a
    // duplicated declaration, where the second copy wins and the first is a
    // lie sitting in the file waiting to be read as the truth.
    const blocks = new Map<string, string>();
    const re = /(?:^:root,\n\[data-ttp-theme="wild"\]|^\[data-ttp-theme="(\w+)"\]) \{([\s\S]*?)\n\}/gm;
    for (let m = re.exec(css); m; m = re.exec(css)) {
      blocks.set(m[1] ?? 'wild', m[2]);
    }
    expect([...blocks.keys()].sort()).toEqual(COATS.map((c) => c.id).sort());

    const names = (body: string) => [...body.matchAll(/--([\w-]+):/g)].map((m) => m[1]);
    const base = names(blocks.get('wild')!);
    const required = [
      'l-bg', 'd-bg', 'l-surface', 'd-surface', 'l-text', 'd-text',
      'l-accent', 'd-accent', 'l-glow', 'd-glow', 'ttp-pill', 'ttp-pill-ink',
    ];

    for (const [coat, body] of blocks) {
      const declared = names(body);
      expect(new Set(declared).size, `${coat} declares a token twice`).toBe(declared.length);
      for (const r of required) {
        expect(declared, `${coat} is missing --${r}`).toContain(r);
      }
      // A coat may not invent a token the base has never heard of: nothing
      // would resolve it when another coat is selected.
      for (const d of declared) {
        expect(base, `--${d} exists only in ${coat}`).toContain(d);
      }
    }
  });
});

describe('copy', () => {
  const en = JSON.parse(read('../i18n/locales/en.json'));
  const fr = JSON.parse(read('../i18n/locales/fr.json'));

  it('names and describes every coat in both locales', () => {
    for (const locale of [en, fr]) {
      const coats = locale.settings.appearance.coats;
      for (const c of COATS) {
        expect(typeof coats?.[c.id]?.name).toBe('string');
        expect(typeof coats?.[c.id]?.desc).toBe('string');
      }
    }
  });

  it('does not leave a coat name untranslated', () => {
    for (const c of COATS) {
      expect(en.settings.appearance.coats[c.id].name)
        .not.toBe(fr.settings.appearance.coats[c.id].name);
    }
  });
});

describe('the event name', () => {
  it('is stable', () => {
    // Other windows listen for this string. Changing it silently desynchronises
    // the pill from Settings until the next launch.
    expect(COAT_EVENT).toBe('ttp-coat-changed');
  });
});
