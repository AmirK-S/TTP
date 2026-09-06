// TTP - Talk To Paste
// What the pill is allowed to say about a paste, and how loudly.
//
// The Rust side stopped lying in Polaris (`src-tauri/src/paste/outcome.rs`,
// `docs/tracing.md` § "`verification`: what the finish line is allowed to
// claim"). `outcome:"pasted"` now means **observed** — somebody read the
// target field back and it changed — and three other terminal states came out
// from behind it. This module is the same join on the frontend: it maps those
// four slugs onto the four things the pill may do.
//
// ── The proportion problem ──────────────────────────────────────────────────
//
// `pasted_unverified` is not an edge case. Measured over the maintainer's
// 548-verification corpus, 219 verifications — 40% — could see nothing at all,
// and separately the verifier settles *after* `dictation.finish` in 78% of
// dictations. So whatever this state looks like, the user is going to see it
// several times a day, on dictations where nothing whatsoever went wrong.
//
// That rules out an error treatment, a colour change, an icon that means
// "warning", and anything that has to be dismissed. It does not rule out
// *saying something*: the defect this whole wave exists to fix is that four
// different facts rendered as one identical frame, and shipping a one-glyph
// difference nobody is ever told about would be honest by the letter and
// concealing in effect.
//
// So the split is by *what the user can act on*:
//
//   * `pasted` — the text is in front of them and we watched it arrive. A
//     mark, no words. Claiming success out loud on the happy path is the same
//     self-congratulation the trace just had removed from it.
//   * `pasted_unverified` — a mark that reads differently, plus four quiet
//     words in the pill's ordinary colour. Nothing to do, nothing to dismiss,
//     gone in a second and a half.
//   * `paste_swallowed` — rare, and their words did not arrive. This one gets
//     the tremble, the warning colour, and the sentence that says where the
//     text still is.
//   * `clipboard_fallback` — unchanged in meaning: the injection never
//     happened and the text is on the clipboard.
//
// The assistive channel is deliberately *not* proportional to the visual one.
// A sighted user can look at their own text field, which is a better verifier
// than any Accessibility read; a screen-reader user cannot. So every outcome,
// including the observed one, carries a full sentence into the live region,
// where the visual treatment carries a mark.

/**
 * The terminal `outcome` slugs from `dictation.finish`.
 *
 * Stable — they are `finish_outcome`'s return values, they are in
 * `docs/tracing.md`, and they are what the Rust side must put on the wire.
 */
export const PASTE_OUTCOMES = [
  'pasted',
  'pasted_unverified',
  'paste_swallowed',
  'clipboard_fallback',
] as const;

export type PasteOutcome = (typeof PASTE_OUTCOMES)[number];

/**
 * Read an outcome off an event payload.
 *
 * Returns `null` for anything unrecognised, **including a missing field**, and
 * that null is load-bearing rather than defensive. Today's Rust emits
 * `emit_progress(app, "complete", "", None)` for all four outcomes; until it
 * carries the slug, the honest frontend answer is "this build did not tell
 * me", which renders as the pre-existing completion and claims nothing. The
 * failure mode we must never have is a missing field defaulting to `pasted`
 * and reinventing the bug in TypeScript.
 */
export function asPasteOutcome(value: unknown): PasteOutcome | null {
  return typeof value === 'string' && (PASTE_OUTCOMES as readonly string[]).includes(value)
    ? (value as PasteOutcome)
    : null;
}

/** Which mark the pill draws. */
export type OutcomeMark = 'sent' | 'arrived' | 'alert';

export interface OutcomeTreatment {
  mark: OutcomeMark;
  /** i18n key for the line drawn in the pill, or `null` for a mark alone. */
  line: string | null;
  /** i18n key for the sentence put into the live region. Never null. */
  announcement: string;
  /** `danger` recolours the pill and adds the tremble. */
  tone: 'active' | 'danger';
  /** How long the frame stays up once it has committed, in ms. */
  holdMs: number;
}

/**
 * The single tick / double tick is not decoration and not a joke.
 *
 * "Sent" versus "delivered" is the one status distinction a very large number
 * of people already read fluently, off exactly this pair of glyphs, and it is
 * precisely the distinction the verifier makes: `paste.result {"ok":true}`
 * means the keystrokes were handed to the window server, and `verdict:
 * "observed"` means somebody watched them arrive. Borrowing an idiom the user
 * already has beats teaching them a new one.
 *
 * `holdMs` is measured from the moment the frame commits, not from
 * `complete`. See `SETTLE_MS` in `usePasteCompletion`.
 */
const TREATMENTS: Record<PasteOutcome, OutcomeTreatment> = {
  // Double tick, no words. 800 ms: `anim-check-pop` runs 350 ms of that, and a
  // mark that is still springing when the frame closes reads as a glitch
  // rather than as an acknowledgement. Rendered at 1x and looked at — 640 ms
  // put the close inside the pop's tail.
  pasted: {
    mark: 'arrived',
    line: null,
    announcement: 'floatingBar.outcome.arrivedAnnouncement',
    tone: 'active',
    holdMs: 800,
  },
  // Single tick and four words, in the pill's ordinary ink at 70% — a caption,
  // not a warning. 1500 ms because four words need reading and this is the one
  // state whose whole job is to be read.
  pasted_unverified: {
    mark: 'sent',
    line: 'floatingBar.outcome.unverified',
    announcement: 'floatingBar.outcome.unverifiedAnnouncement',
    tone: 'active',
    holdMs: 1500,
  },
  // The rare one, and the only one where the user has lost something. Same
  // 4 s the error frame has always used, because it is the same class of
  // event, and it names the recovery: `add_history_entry` has already run by
  // the time the verdict lands, so the text is in Settings → History with a
  // replay button beside it.
  paste_swallowed: {
    mark: 'alert',
    line: 'floatingBar.outcome.swallowed',
    announcement: 'floatingBar.outcome.swallowedAnnouncement',
    tone: 'danger',
    holdMs: 4000,
  },
  // Reached through the `error` stage in practice — Rust routes a failed
  // injection there with `error.paste_failed` — but kept total over the four
  // slugs so a payload carrying it can never fall through to a success frame.
  clipboard_fallback: {
    mark: 'alert',
    line: 'floatingBar.outcome.clipboard',
    announcement: 'floatingBar.outcome.clipboardAnnouncement',
    tone: 'danger',
    holdMs: 4000,
  },
};

export function treatmentFor(outcome: PasteOutcome): OutcomeTreatment {
  return TREATMENTS[outcome];
}
