// TTP - Talk To Paste
// The four outcomes, and the proportion between them.
//
// Two classes of thing are pinned here. The first is the vocabulary: these
// slugs are `finish_outcome`'s return values in
// `src-tauri/src/paste/outcome.rs`, they are tabled in `docs/tracing.md`, and
// a rename on either side must break something rather than silently render a
// success frame for a swallowed paste.
//
// The second is the *design decision*, expressed as invariants rather than as
// prose in a comment nobody re-reads. `pasted_unverified` is going to be a
// large minority of every user's dictations — 219 blind verifications in 548,
// and a verdict that settles after the finish line in 78% of runs — so the
// rules that keep it from becoming an alarm are load-bearing and a later
// change that quietly promotes it to `danger`, or gives the observed case a
// sentence, should turn something red.

import { describe, expect, it } from 'vitest';
import en from '../i18n/locales/en.json';
import fr from '../i18n/locales/fr.json';
import {
  PASTE_OUTCOMES,
  asPasteOutcome,
  treatmentFor,
  type PasteOutcome,
} from './pasteOutcome';

/** Resolve a dotted i18next path against a locale object. */
function leaf(locale: unknown, dotted: string): unknown {
  return dotted.split('.').reduce<unknown>(
    (cur, part) =>
      cur && typeof cur === 'object' ? (cur as Record<string, unknown>)[part] : undefined,
    locale,
  );
}

describe('the outcome vocabulary', () => {
  it('is exactly the four slugs Rust can finish with', () => {
    // `finish_outcome` in src-tauri/src/paste/outcome.rs. `aborted` is not
    // here on purpose: a dictation that produced no text never reaches a
    // paste, and the pill shows an error for it.
    expect([...PASTE_OUTCOMES]).toEqual([
      'pasted',
      'pasted_unverified',
      'paste_swallowed',
      'clipboard_fallback',
    ]);
  });

  it('reads a slug off a payload', () => {
    expect(asPasteOutcome('pasted')).toBe('pasted');
    expect(asPasteOutcome('paste_swallowed')).toBe('paste_swallowed');
  });

  /* The regression that would reimplement the bug in TypeScript. Rust today
     emits `emit_progress(app, "complete", "", None)` for all four outcomes;
     if a missing field defaulted to `pasted`, the frontend would resume
     claiming an observation it does not have. */
  it('never invents an outcome for a payload that carries none', () => {
    for (const absent of [undefined, null, '', 'pasted ', 'PASTED', 'ok', true, 0, {}]) {
      expect(asPasteOutcome(absent)).toBeNull();
    }
  });
});

describe('proportion', () => {
  it('does not treat an unverified paste as a failure', () => {
    const t = treatmentFor('pasted_unverified');
    // Not the danger tone — so no red pill and, via `isDanger` in
    // FloatingBar, no tremble. This is the common case and nothing went
    // wrong in it.
    expect(t.tone).toBe('active');
    expect(t.mark).toBe('sent');
  });

  it('says nothing out loud when it watched the text arrive', () => {
    // No visible line on the happy path. Announcing success on every
    // dictation is the same self-congratulation the trace had removed from
    // it, and the user is looking at their own text.
    expect(treatmentFor('pasted').line).toBeNull();
  });

  it('gives the unverified state words, because a glyph alone would conceal', () => {
    expect(treatmentFor('pasted_unverified').line).not.toBeNull();
  });

  it('reserves the danger tone for the two states where text is missing', () => {
    const danger = PASTE_OUTCOMES.filter((o) => treatmentFor(o).tone === 'danger');
    expect(danger).toEqual(['paste_swallowed', 'clipboard_fallback']);
  });

  it('keeps every frame short, and the quiet ones shortest', () => {
    const hold = (o: PasteOutcome) => treatmentFor(o).holdMs;
    // Ordered by how much there is to read, not by how bad it is.
    expect(hold('pasted')).toBeLessThan(hold('pasted_unverified'));
    expect(hold('pasted_unverified')).toBeLessThan(hold('paste_swallowed'));
    // Nothing lingers. A frame that outstayed the next hotkey press would be
    // a state the user has to wait out, which is a demand.
    expect(hold('pasted_unverified')).toBeLessThanOrEqual(1500);
  });
});

describe('the strings exist in both languages', () => {
  // `check-i18n-keys-used.mjs` cannot see these: they are table entries, not
  // literal `t('…')` call sites, so a typo would ship as a raw key painted on
  // the pill. This is that check.
  it.each([...PASTE_OUTCOMES])('%s', (outcome) => {
    const t = treatmentFor(outcome);
    for (const key of [t.line, t.announcement]) {
      if (key === null) continue;
      expect(typeof leaf(en, key), `${key} missing from en.json`).toBe('string');
      expect(typeof leaf(fr, key), `${key} missing from fr.json`).toBe('string');
      expect(leaf(en, key)).not.toBe('');
      expect(leaf(fr, key)).not.toBe('');
    }
  });

  it('does not ship the French as a copy of the English', () => {
    for (const outcome of PASTE_OUTCOMES) {
      const { line, announcement } = treatmentFor(outcome);
      for (const key of [line, announcement]) {
        if (key === null) continue;
        expect(leaf(fr, key), `${key} was not rewritten`).not.toBe(leaf(en, key));
      }
    }
  });

  /* Every outcome reaches assistive tech as a whole sentence, including the
     one that draws no words. A sighted user can check their own text field;
     a screen-reader user cannot, so the live region is where the asymmetry is
     paid back. */
  it('announces every outcome, including the silent one', () => {
    for (const outcome of PASTE_OUTCOMES) {
      expect(treatmentFor(outcome).announcement).toMatch(/^floatingBar\.outcome\./);
    }
  });
});
