// TTP - What actually landed in the field
//
// `classify` answers "did the field change?". That is not the question the
// user cares about, which is "is my text there?", and on 2026-09-26 the two
// came apart: 156 characters typed into the Claude desktop composer, 21
// arrived — the last character of each 16-character chunk and then the whole
// last chunk, "nseee ene, simplifie." — and the verifier said `observed`
// because the field had grown. The message was sent like that.
//
// The verifier already reads the field's *text* on most targets (AXValue or
// AXStringForRange). This module compares that text against what we typed,
// so a garbled landing is named for what it is, and it hands the repair the
// exact range to replace.
//
// Pure — no Accessibility calls — so every shape seen in the trace is a test.

/// How much of the dictated text is in the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landing {
    /// Everything we typed is there (allowing for what text fields do to
    /// text on the way in: smart quotes, newline flavours, our own ZWSP), or
    /// the field changed in a way that is not scraps of our text.
    Complete,
    /// Something arrived and it is not the text. `range` is where it sits in
    /// the field, in UTF-16 units — the unit `AXSelectedTextRange` speaks.
    Partial { landed: String, range: (usize, usize) },
    /// The field is as it was.
    Absent,
}

/// A landing counts as complete from this share of the expected characters.
///
/// Not 100%: native fields rewrite on the way in (TextEdit turns ' into ’,
/// some composers trim a trailing space), and calling that "incomplete" would
/// retype text the user already has. The failure this exists for lands a
/// fraction — 21 of 156, 55 of 70, 305 of 455 in the Claude app — nowhere near.
const COMPLETE_SHARE: f64 = 0.9;

/// Strip what the injection or the field adds without the user having said it.
///
/// * U+200B: `injection_chunks` prefixes chunks that start with a newline.
/// * `\r\n` / `\r`: fields disagree on newlines.
/// * Any run of whitespace, NBSP included, compares as one space: polish puts
///   NBSP before French punctuation and some fields store a plain space.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for ch in text.chars() {
        if ch == '\u{200B}' {
            continue;
        }
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out.trim().to_string()
}

/// The span of `after` that is not in `before`: longest common prefix and
/// suffix removed. Returns the inserted text and its UTF-16 range in `after`.
///
/// Typing over a selection or into a placeholder still yields the right span:
/// the prefix and suffix are what survived, the middle is what we put there.
pub fn inserted(before: &str, after: &str) -> (String, (usize, usize)) {
    let b: Vec<char> = before.chars().collect();
    let a: Vec<char> = after.chars().collect();
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let middle: String = a[pre..a.len() - suf].iter().collect();
    let start: usize = a[..pre].iter().map(|c| c.len_utf16()).sum();
    let len: usize = middle.encode_utf16().count();
    (middle, (start, len))
}

/// Compare one read of the field against what we typed.
pub fn assess(expected: &str, before: &str, after: &str) -> Landing {
    if before == after {
        return Landing::Absent;
    }
    let want = normalize(expected);
    if want.is_empty() || normalize(after).contains(&want) {
        return Landing::Complete;
    }
    let (landed, range) = inserted(before, after);
    let got = normalize(&landed);
    if got.is_empty() {
        // The field changed without gaining anything: text was removed. Not
        // ours to judge — the user, or the app, edited.
        return Landing::Absent;
    }
    let share = got.chars().count() as f64 / want.chars().count() as f64;
    if share >= COMPLETE_SHARE {
        return Landing::Complete;
    }
    // Scraps *of our text*: every character that arrived is one we sent, in
    // order. Lost chunks, a lost head, "nseee ene, simplifie." all qualify. A
    // terminal redrawing its prompt, or a composer that reformats, puts
    // characters we never typed into the diff — that is the app's business,
    // not a failed injection, and it must never trigger a retype.
    if !is_subsequence(&got, &want) {
        return Landing::Complete;
    }
    Landing::Partial { landed, range }
}

fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut hay = hay.chars();
    needle.chars().all(|c| hay.any(|h| h == c))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENT: &str = "Je vais te demander des promptes, donc des choses à copier et des choses à coller. D'accord. Ne les surcomplique pas, juste donne l'idée globale, simplifie.";

    /// 0024-8368, 2026-09-26: the incident this module exists for.
    #[test]
    fn the_claude_composer_incident_is_partial() {
        match assess(SENT, "", "nseee ene, simplifie.") {
            Landing::Partial { landed, range } => {
                assert_eq!(landed, "nseee ene, simplifie.");
                assert_eq!(range, (0, 21));
            }
            other => panic!("{other:?}"),
        }
    }

    /// Reproduced by the bench: focus returned to the app 0 ms before typing,
    /// the first two chunks vanished.
    #[test]
    fn a_lost_head_is_partial_and_the_range_covers_what_landed() {
        let after = &SENT[32..];
        match assess(SENT, "", after) {
            Landing::Partial { range, .. } => assert_eq!(range, (0, after.encode_utf16().count())),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_full_landing_is_complete() {
        assert_eq!(assess(SENT, "", SENT), Landing::Complete);
    }

    #[test]
    fn text_typed_into_existing_text_is_found_where_it_landed() {
        let before = "Salut. \n";
        let after = format!("Salut. {SENT}\n");
        assert_eq!(assess(SENT, before, &after), Landing::Complete);
        let (landed, range) = inserted(before, &format!("Salut. nseee\n"));
        assert_eq!(landed, "nseee");
        assert_eq!(range, (7, 5));
    }

    /// The empty Claude composer reads as one character, and typing replaces
    /// a placeholder in some fields. Neither is a partial landing.
    #[test]
    fn a_replaced_placeholder_is_complete() {
        assert_eq!(assess("bonjour à tous", "Reply to Claude…", "bonjour à tous"), Landing::Complete);
        assert_eq!(assess("bonjour à tous", "\n", "bonjour à tous\n"), Landing::Complete);
    }

    #[test]
    fn what_fields_do_to_text_is_not_a_loss() {
        // Smart quote, NBSP before punctuation, CRLF, our own ZWSP.
        assert_eq!(assess("l'idée : oui", "", "l’idée : oui"), Landing::Complete);
        assert_eq!(assess("a\nb", "", "a\r\nb"), Landing::Complete);
        assert_eq!(assess("a\nb", "", "a\u{200B}\nb"), Landing::Complete);
    }

    #[test]
    fn an_unchanged_field_is_absent_not_partial() {
        assert_eq!(assess(SENT, "x", "x"), Landing::Absent);
        // Shrunk: somebody deleted, we did not type that.
        assert_eq!(assess(SENT, "abc", "ab"), Landing::Absent);
    }

    /// A TUI redrawing around the text puts characters we never typed into the
    /// diff. Not a loss, and above all not something to retype over.
    #[test]
    fn foreign_characters_are_the_app_not_a_loss() {
        assert_eq!(assess(SENT, "│ > │", "│ > Je vais │\n│ te demander │"), Landing::Complete);
    }

    /// AX ranges are UTF-16: an emoji before the landing shifts it by two.
    #[test]
    fn ranges_are_utf16() {
        let (_, range) = inserted("🎉 ", "🎉 abc");
        assert_eq!(range, (3, 3));
    }
}
