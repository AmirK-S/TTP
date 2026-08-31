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
import {
  COATS,
  COAT_EVENT,
  DEFAULT_COAT_ID,
  applyCoat,
  effectiveCoat,
  getCoatSnapshot,
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

beforeEach(() => {
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
