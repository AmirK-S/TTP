// TTP - What the pipeline is allowed to *claim* about a paste
//
// `paste.result {"ok":true}` means CGEventPost returned. It is a self-report,
// and for 40% of the maintainer's corpus it was the only thing behind
// `dictation.finish {"outcome":"pasted"}`. This module is the join between the
// evidence (`accessibility::classify`) and the words the trace is allowed to
// use.
//
// ── Why a slot ──────────────────────────────────────────────────────────────
//
// Verification cannot run on the dictation's critical path. The target
// consumes our synthetic events on its own run loop; measured across the
// 548-verification corpus, `paste.verify` lands a median of 44 ms after
// `dictation.finish` and a 90th percentile of 616 ms, and the state machine
// stays in `Processing` until `process_recording` returns — so every
// millisecond waited here is a millisecond of dead hotkey. Sleeping to collect
// the answer is not an option and never was.
//
// So the verifier settles asynchronously into a shared slot, and the finish
// path *reads* it without blocking. Whatever is in the slot at that instant is
// the honest answer at that instant: usually `pending`, sometimes already
// decided, and — for a target with no readable baseline — decided before the
// injection even happened.

use super::{PasteVerdict, Verification};
use std::sync::{Arc, Mutex};

/// Where `spawn_paste_verification` leaves its answer for the finish path.
///
/// `None` means "has not concluded yet", which is a real and common state, not
/// a missing value: 78% of corpus verifications concluded after
/// `dictation.finish` had already been written.
pub type PasteVerdictSlot = Arc<Mutex<Option<Verification>>>;

/// Publish a verdict for the finish path to read.
pub fn record_verdict(slot: &PasteVerdictSlot, verification: Verification) {
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(verification);
    }
}

/// What the verifier has concluded *so far*. Never blocks.
///
/// A poisoned mutex degrades to `None` — i.e. to `pending` — rather than
/// panicking. It cannot occur in a shipped binary (`panic = "abort"` means a
/// panic never unwinds out of the lock), and if it somehow did, reporting "we
/// do not know yet" is the correct answer anyway.
pub fn read_verdict(slot: &PasteVerdictSlot) -> Option<Verification> {
    slot.lock().ok().and_then(|guard| *guard)
}

/// The one word `ui.completed` and `dictation.finish` are allowed to say about
/// whether anybody actually saw the keystrokes land.
///
/// Stable slugs — they are grepped and they are in `docs/tracing.md`:
///
/// * `observed` — a before/after pair of reads disagreed. The characters
///   landed. This is the only value that licenses `outcome:"pasted"`.
/// * `swallowed` — the target was readable and did not change. The shape the
///   stuck-Globe bug produces.
/// * `ax_unreadable` — there was no baseline to compare against, so no evidence
///   was ever going to arrive. **Not** a failure and not a success: the text
///   very probably landed and nothing looked.
/// * `pending` — the verifier has not concluded yet. The answer is coming on
///   the same trace id, in `paste.verify`.
/// * `inconclusive` — the verifier looked at both sides and still could not
///   tell (`read_back_failed`, `length_unchanged`, `shape_changed`).
/// * `not_pasted` — the injection failed or was skipped. Nothing to verify.
///
/// `verifiable` is the pre-injection answer to "is there a baseline at all",
/// and it is what lets this function be honest *before* the verifier has run.
pub fn describe_verification(
    paste_success: bool,
    verifiable: bool,
    settled: Option<Verification>,
) -> &'static str {
    if !paste_success {
        return "not_pasted";
    }
    // A concluded verdict wins over the pre-injection guess. The two never
    // disagree — a non-observable baseline makes `classify` return
    // `no_baseline` — but reading the evidence first is the right precedence.
    if let Some(v) = settled {
        return match v.verdict {
            PasteVerdict::Observed => "observed",
            PasteVerdict::Swallowed => "swallowed",
            // "we could not look" and "we looked and it was ambiguous" need
            // different fixes, so they are not the same word.
            PasteVerdict::Unverified => {
                if v.reason == "no_baseline" {
                    "ax_unreadable"
                } else {
                    "inconclusive"
                }
            }
        };
    }
    if !verifiable {
        // Decided before injection: the target had nothing readable to compare
        // against. Saying `pending` here would promise evidence that is not
        // coming.
        return "ax_unreadable";
    }
    "pending"
}

/// The terminal `outcome` slug for `dictation.finish`.
///
/// The whole point: **`pasted` now means observed.** It used to mean "we
/// posted the events and nothing errored", which is what let 219 blind pastes
/// across the corpus — including `0112-bf80`, the one dictation where Fn/Globe
/// was demonstrably held at injection — report themselves as successes.
///
/// * `pasted` — observed. Someone read the target and it changed.
/// * `paste_swallowed` — observed *not* to have landed.
/// * `pasted_unverified` — posted, not observed. Covers both "the evidence is
///   still in flight" and "there was never going to be any".
/// * `clipboard_fallback` — the injection itself did not happen. Unchanged
///   meaning; the text is on the clipboard.
pub fn finish_outcome(paste_success: bool, verification: &str) -> &'static str {
    if !paste_success {
        return "clipboard_fallback";
    }
    match verification {
        "observed" => "pasted",
        "swallowed" => "paste_swallowed",
        // `pending`, `ax_unreadable`, `inconclusive`. All three mean the same
        // thing to a reader of the terminal line: nobody saw it land *yet*.
        // `verification` on the same line says which.
        _ => "pasted_unverified",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paste::{classify, FocusSnapshot, FocusSource};

    fn verified(verdict: PasteVerdict, evidence: &'static str, reason: &'static str) -> Verification {
        Verification { verdict, evidence, reason }
    }

    fn text(t: &str) -> FocusSnapshot {
        FocusSnapshot {
            text: Some(t.to_string()),
            chars: Some(t.chars().count()),
            source: FocusSource::Value,
            ax_err: 0,
        }
    }

    #[test]
    fn a_fresh_slot_reads_as_pending() {
        let slot: PasteVerdictSlot = Arc::new(Mutex::new(None));
        assert_eq!(read_verdict(&slot), None);
        assert_eq!(describe_verification(true, true, read_verdict(&slot)), "pending");
    }

    #[test]
    fn a_recorded_verdict_is_readable_without_blocking() {
        let slot: PasteVerdictSlot = Arc::new(Mutex::new(None));
        record_verdict(&slot, classify(&text("bon"), &text("bonjour")));
        assert_eq!(read_verdict(&slot).unwrap().verdict, PasteVerdict::Observed);
        assert_eq!(describe_verification(true, true, read_verdict(&slot)), "observed");
    }

    /// The regression the workstream exists for: posting the events is not
    /// seeing them land, and the terminal line must stop conflating them.
    #[test]
    fn only_an_observation_licenses_pasted() {
        assert_eq!(finish_outcome(true, "observed"), "pasted");
        for v in ["pending", "ax_unreadable", "inconclusive"] {
            assert_eq!(finish_outcome(true, v), "pasted_unverified", "{v}");
        }
    }

    /// `0112-bf80`: readable-false, Fn/Globe held, reported `outcome:"pasted"`.
    /// The same inputs must now produce the honest third state.
    #[test]
    fn a_blind_target_finishes_as_pasted_unverified() {
        let verification = describe_verification(true, false, None);
        assert_eq!(verification, "ax_unreadable");
        assert_eq!(finish_outcome(true, verification), "pasted_unverified");
    }

    /// And it must not be reported as a failure either. `ax_unreadable` is not
    /// `clipboard_fallback`: the text is not sitting on the clipboard waiting
    /// to be pasted, it almost certainly went in.
    #[test]
    fn unverified_is_not_a_paste_failure() {
        assert_ne!(finish_outcome(true, "ax_unreadable"), "clipboard_fallback");
        assert_eq!(finish_outcome(false, "not_pasted"), "clipboard_fallback");
    }

    #[test]
    fn a_swallow_is_neither_pasted_nor_unverified() {
        let v = describe_verification(true, true, Some(classify(&text("bonjour"), &text("bonjour"))));
        assert_eq!(v, "swallowed");
        assert_eq!(finish_outcome(true, v), "paste_swallowed");
    }

    /// A settled verdict overrides the pre-injection guess, in both directions.
    #[test]
    fn evidence_beats_the_pre_injection_guess() {
        let observed = Some(verified(PasteVerdict::Observed, "length", ""));
        assert_eq!(describe_verification(true, false, observed), "observed");
        let blind = Some(verified(PasteVerdict::Unverified, "none", "no_baseline"));
        assert_eq!(describe_verification(true, true, blind), "ax_unreadable");
    }

    /// "We could not look" and "we looked and could not tell" are different
    /// facts with different fixes, so they get different words.
    #[test]
    fn ambiguous_evidence_is_not_the_same_as_no_evidence() {
        let ambiguous = Some(verified(PasteVerdict::Unverified, "length", "length_unchanged"));
        assert_eq!(describe_verification(true, true, ambiguous), "inconclusive");
        let lost = Some(verified(PasteVerdict::Unverified, "none", "read_back_failed"));
        assert_eq!(describe_verification(true, true, lost), "inconclusive");
        let none = Some(verified(PasteVerdict::Unverified, "none", "no_baseline"));
        assert_eq!(describe_verification(true, true, none), "ax_unreadable");
    }

    /// A failed injection has nothing to verify, whatever the verifier's slot
    /// happens to hold.
    #[test]
    fn a_failed_injection_is_never_described_as_evidence() {
        let observed = Some(verified(PasteVerdict::Observed, "text", ""));
        assert_eq!(describe_verification(false, true, observed), "not_pasted");
        assert_eq!(finish_outcome(false, "not_pasted"), "clipboard_fallback");
    }

    /// These strings are in `docs/tracing.md` and in the analyser's finding
    /// rules. Pin them.
    #[test]
    fn trace_slugs_are_stable() {
        assert_eq!(describe_verification(true, true, None), "pending");
        assert_eq!(describe_verification(true, false, None), "ax_unreadable");
        assert_eq!(describe_verification(false, false, None), "not_pasted");
        assert_eq!(finish_outcome(true, "observed"), "pasted");
        assert_eq!(finish_outcome(true, "swallowed"), "paste_swallowed");
        assert_eq!(finish_outcome(true, "pending"), "pasted_unverified");
        assert_eq!(finish_outcome(false, "anything"), "clipboard_fallback");
    }
}
