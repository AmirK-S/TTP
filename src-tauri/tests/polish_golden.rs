// Integration tests for the Polaris polish pipeline. Hits the live Groq model
// named by `polish::MODEL` and verifies anti-injection + self-correction + intent
// classification behavior against the golden fixture set.
//
// ── The live run is opt-in TWICE, on purpose ─────────────────────────────
//
// `#[ignore]` plus `GROQ_API_KEY` was not enough. On 2026-09-02 a run fired 54
// calls, exhausted this account's rate tier, and wrote a burst of
// `polish.attempt` 429s into the maintainer's own `ttp-trace.log` that was then
// read as a production incident. `cargo test -- --ignored` is a thing people
// run for unrelated reasons (see the keychain test in `licensing::storage`),
// and one careless invocation costs real money and contaminates the corpus the
// harvest window exists to collect.
//
// So the live test also requires `TTP_POLISH_LIVE=1`. Nothing infers it,
// nothing defaults it. Run it deliberately:
//
//   TTP_POLISH_LIVE=1 GROQ_API_KEY=gsk_... \
//     cargo test --test polish_golden -- --ignored --nocapture
//
// `fixture_intents_agree_with_the_prompt` below is the part that runs for free:
// it is not `#[ignore]`d, makes no network call, and catches the class of bug
// that cost those 54 calls to discover.
//
// In CI, the live test can be wired into a periodic job (not per-PR) since
// polish behavior shifts with model updates and we want canary alerts.
//
// ── What `expected_intent` is worth, decided 2026-09-05 ──────────────────
//
// On 2026-09-02 the suite ran 11/27. FIFTEEN of the sixteen failures were
// intent-label-only: the polished text — the thing the user actually receives —
// was correct every time.
//
// `intent` HAS NO CONSUMER. Verified by grep, not assumed: the five sites that
// read `PolishResult::intent` are `pipeline.rs:1795, 1797, 1819, 1823, 1827`,
// and every one of them formats it into a trace field, a log line or a Sentry
// breadcrumb. No branch anywhere in the crate reads it. The docstring that
// promised downstream routing has been corrected to say so.
//
// Three options were on the table. The chosen one, and why:
//
//   * DELETE the intent assertions. Rejected. The label is free — it arrives in
//     the same JSON response as `polished`, so checking it costs nothing — and
//     a wholesale shift in labels is a real signal that the model behind
//     `polish::MODEL` changed under us, which is the failure that cost eight
//     days of unpolished dictation in August.
//
//   * Give `intent` a CONSUMER. Rejected, and firmly. Building a routing
//     feature so that an existing test has something to assert against inverts
//     the dependency. `docs/engineering-standards.md` §1.5 is about exactly
//     this shape: an abstraction that exists for a caller who never arrived.
//
//   * KEEP them, NON-BLOCKING. Chosen. An intent mismatch is printed, counted
//     and summarised, and never fails the run. Only `polished` can fail it.
//     Failing a suite on a label nobody reads teaches people that red means
//     nothing, which is how a suite stops being read at all.
//
// The teeth moved offline instead. Ten of the thirteen `form_field` returns
// were exactly what `POLISH_SYSTEM_PROMPT` specifies — those fixtures were
// contradicting the prompt, not catching misbehaviour. Three of them
// (`fr_repetition_emphasis`, `en_repetition_disfluency`,
// `fr_profanity_preserved`) had no competing rule and were simply wrong; they
// now say `form_field`. The other twelve collide with the `raw_prompt` rule,
// and the prompt states NO PRECEDENCE between "short utterance (under 8 words)"
// and "imperative verb, question, or instruction-shaped" — so those carry an
// `intent_advisory` naming the collision. `fixture_intents_agree_with_the_prompt`
// makes that bookkeeping mandatory and self-maintaining, offline.
//
// Two things are deliberately NOT fixed here because they live in
// `transcription/polish.rs`, which this workstream does not own:
//   * The precedence gap in `POLISH_SYSTEM_PROMPT` described above.
//   * `fr_self_correction_lexical_immediate` — "mets-moi un rendez-vous pour
//     demain, après-demain" is labelled the user's original bug report, rule 5
//     of the prompt names this exact string, and the model does not apply it.
//     That is a real product defect and the one genuine golden failure.
//
// PACING. Groq's on-demand tier caps this account at 8000 tokens per minute for
// `openai/gpt-oss-120b`, and one fixture costs ~1100-1240 tokens of that budget
// (measured 2026-09-02 from the 429 bodies, which report `Used` and `Requested`
// per call). That is ~6.5 fixtures per minute. Fired back to back, the suite
// exhausts the budget at fixture 6 and every call after it 429s. Before this
// pacing existed the run reported "Pass rate 18.5%" and 17 of the 22 failures
// were rate limits, not golden misses — the file was blaming the model for the
// harness. FIXTURE_SPACING_MS is 60_000 / 6.5 rounded up, and rate-limit
// failures are now counted and reported separately from assertion failures so
// the two can never be confused again.
const FIXTURE_SPACING_MS: u64 = 9_500;

use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct GoldenFile {
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    raw: String,
    #[serde(default)]
    expected_contains: Option<String>,
    #[serde(default)]
    expected_contains_all: Option<Vec<String>>,
    #[serde(default)]
    expected_contains_any: Option<Vec<String>>,
    #[serde(default)]
    expected_does_not_contain: Option<Vec<String>>,
    #[serde(default)]
    expected_does_not_contain_phrases: Option<Vec<String>>,
    #[serde(default)]
    expected_does_not_match: Option<String>,
    #[serde(default)]
    expected_intent: Option<String>,
    /// Why this fixture's `expected_intent` disagrees with the `form_field`
    /// rule in `POLISH_SYSTEM_PROMPT`.
    ///
    /// A prose reason rather than a bool, so that adding the flag forces the
    /// author to name WHICH two prompt rules collide. Required — and required
    /// to be absent otherwise — by `fixture_intents_agree_with_the_prompt`.
    #[serde(default)]
    intent_advisory: Option<String>,
}

/// The `form_field` rule from `POLISH_SYSTEM_PROMPT`, restated as code:
/// "short utterance (under 8 words) without sentence-ending punctuation".
///
/// This is a duplicate of a rule whose home is a prompt string in
/// `transcription/polish.rs`, and duplication is the thing §1.3 of the
/// standards warns about. It earns its place anyway: the prompt is prose sent
/// to a model, so nothing else in this repository can check that a fixture
/// agrees with it, and the disagreement it catches took 27 live API calls to
/// discover the first time. If the prompt's wording changes, this function and
/// the citation above it change with it.
fn form_field_length_rule_applies(raw: &str) -> bool {
    let words = raw.split_whitespace().count();
    let ends_a_sentence = raw
        .trim_end()
        .chars()
        .last()
        .is_some_and(|c| matches!(c, '.' | '!' | '?'));
    words < 8 && !ends_a_sentence
}

fn load_fixtures() -> Vec<Fixture> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("polish_golden.json");
    let content = fs::read_to_string(&path).expect("read golden fixtures");
    let file: GoldenFile = serde_json::from_str(&content).expect("parse golden fixtures");
    file.fixtures
}

/// Assertions over `polished` — the text the user actually receives. These are
/// the only ones that can fail the run.
fn check_polished(fix: &Fixture, polished: &str) -> Result<(), String> {
    if let Some(expected) = &fix.expected_contains {
        if !polished.to_lowercase().contains(&expected.to_lowercase()) {
            return Err(format!(
                "expected_contains failed: '{}' not in '{}'",
                expected, polished
            ));
        }
    }
    if let Some(all) = &fix.expected_contains_all {
        for token in all {
            if !polished.to_lowercase().contains(&token.to_lowercase()) {
                return Err(format!(
                    "expected_contains_all failed: '{}' not in '{}'",
                    token, polished
                ));
            }
        }
    }
    if let Some(any) = &fix.expected_contains_any {
        let polished_lc = polished.to_lowercase();
        if !any.iter().any(|t| polished_lc.contains(&t.to_lowercase())) {
            return Err(format!(
                "expected_contains_any failed: none of {:?} in '{}'",
                any, polished
            ));
        }
    }
    if let Some(forbidden) = &fix.expected_does_not_contain {
        let polished_lc = polished.to_lowercase();
        for token in forbidden {
            if polished_lc.contains(&token.to_lowercase()) {
                return Err(format!(
                    "expected_does_not_contain failed: '{}' present in '{}'",
                    token, polished
                ));
            }
        }
    }
    if let Some(phrases) = &fix.expected_does_not_contain_phrases {
        let polished_lc = polished.to_lowercase();
        for phrase in phrases {
            if polished_lc.contains(&phrase.to_lowercase()) {
                return Err(format!(
                    "expected_does_not_contain_phrases failed: '{}' present in '{}'",
                    phrase, polished
                ));
            }
        }
    }
    if let Some(pattern) = &fix.expected_does_not_match {
        let re = regex::Regex::new(pattern).map_err(|e| format!("bad regex: {}", e))?;
        if re.is_match(polished) {
            return Err(format!(
                "expected_does_not_match failed: pattern '{}' matched '{}'",
                pattern, polished
            ));
        }
    }
    Ok(())
}

/// The `intent` label. Reported, never fatal — see the header for why.
fn check_intent(fix: &Fixture, intent: &str) -> Option<String> {
    let expected = fix.expected_intent.as_ref()?;
    if intent == expected {
        return None;
    }
    Some(format!(
        "expected_intent={} got intent={}{}",
        expected,
        intent,
        match &fix.intent_advisory {
            Some(reason) => format!(" [advisory: {}]", reason),
            None => String::new(),
        }
    ))
}

#[tokio::test]
#[ignore]
async fn polish_golden_fixtures() {
    // The second gate. `#[ignore]` alone did not stop a `-- --ignored` run from
    // spending 54 live calls and polluting the trace corpus; see the header.
    assert_eq!(
        std::env::var("TTP_POLISH_LIVE").as_deref(),
        Ok("1"),
        "refusing to spend live Groq calls: set TTP_POLISH_LIVE=1 to mean it"
    );
    let api_key = std::env::var("GROQ_API_KEY")
        .expect("GROQ_API_KEY env var must be set to run golden tests");
    let fixtures = load_fixtures();
    let mut failures = Vec::new();
    // Intent mismatches. Collected and printed, never fatal — `intent` reaches
    // the trace, the log and Sentry and nothing else.
    let mut intent_misses = Vec::new();
    // Transport/rate-limit errors are tracked apart from golden misses. A 429 is
    // a statement about the account's tier, not about the model's behaviour, and
    // folding the two together is how this suite last reported an 18.5% pass
    // rate that told nobody anything.
    let mut api_errors = Vec::new();

    for (i, fix) in fixtures.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(FIXTURE_SPACING_MS)).await;
        }
        let result = ttp_lib::transcription::polish::polish_text(&api_key, &fix.raw).await;
        match result {
            Ok(polish_result) => {
                // Map Intent enum to snake_case strings matching the fixture file.
                let intent_str = match polish_result.intent {
                    ttp_lib::transcription::polish::Intent::RawPrompt => "raw_prompt",
                    ttp_lib::transcription::polish::Intent::Code => "code",
                    ttp_lib::transcription::polish::Intent::ListOrEnum => "list_or_enum",
                    ttp_lib::transcription::polish::Intent::FormField => "form_field",
                    ttp_lib::transcription::polish::Intent::NaturalText => "natural_text",
                };

                if let Some(miss) = check_intent(fix, intent_str) {
                    intent_misses.push(format!("{}: {}", fix.id, miss));
                }

                match check_polished(fix, &polish_result.polished) {
                    Ok(()) => {
                        println!(
                            "✅ {} | intent={} | polished='{}'",
                            fix.id, intent_str, polish_result.polished
                        );
                    }
                    Err(e) => {
                        println!(
                            "❌ {} | intent={} | polished='{}' | {}",
                            fix.id, intent_str, polish_result.polished, e
                        );
                        failures.push(format!("{}: {}", fix.id, e));
                    }
                }
            }
            Err(e) => {
                println!("⚠️  {} | polish_text errored: {}", fix.id, e);
                api_errors.push(format!("{}: polish_text error: {}", fix.id, e));
            }
        }
    }

    let total = fixtures.len();
    let answered = total - api_errors.len();
    let passed = answered - failures.len();
    println!(
        "\n=== Polish golden results: {}/{} passed ({} of {} fixtures got an answer) ===",
        passed, answered, answered, total
    );

    // Printed even when empty, so that "no intent section" can never be read as
    // "intent was not checked". A trace that only speaks up on failure cannot
    // tell a clean result from an absent one.
    println!(
        "\nIntent labels: {}/{} matched ({} mismatch(es)). NOT a pass criterion — \
         `intent` has no consumer; see this file's header.",
        answered - intent_misses.len(),
        answered,
        intent_misses.len()
    );
    for m in &intent_misses {
        println!("  · {}", m);
    }

    if !api_errors.is_empty() {
        println!("\nAPI errors (not golden failures — the model never answered):");
        for e in &api_errors {
            println!("  - {}", e);
        }
    }

    // An unanswered fixture proves nothing either way, so the pass rate is over
    // the answered set. But too few answers means the run itself is worthless,
    // and silently reporting "3/3 passed" out of 27 fixtures would be exactly
    // the green-is-not-done failure this suite exists to catch.
    assert!(
        answered * 4 >= total * 3,
        "only {}/{} fixtures got an answer from the API; run is inconclusive, not green",
        answered,
        total
    );

    if !failures.is_empty() {
        let pass_rate = passed as f32 / answered as f32;
        println!("\nFailures:");
        for f in &failures {
            println!("  - {}", f);
        }
        // 90% pass rate threshold — accounts for LLM nondeterminism on edge
        // cases. Hard failures (jailbreak, length anomaly) should always
        // pass; we allow occasional miss on ambiguous self-corrections.
        assert!(
            pass_rate >= 0.90,
            "Pass rate {:.1}% below 90% threshold",
            pass_rate * 100.0
        );
    }
}

/// Every fixture's `expected_intent` must either agree with
/// `POLISH_SYSTEM_PROMPT`'s `form_field` rule, or say in writing why it does
/// not. Offline, no API key, no network, milliseconds.
///
/// This is the check that was missing. The contradiction it enforces was
/// discovered on 2026-09-02 by spending 27 live Groq calls and reading the
/// output by hand: ten of thirteen `form_field` returns were the prompt being
/// obeyed, and the fixtures were wrong. Nothing about that needed a model.
///
/// The assertion runs in BOTH directions on purpose. A missing advisory means a
/// fixture is quietly contradicting the prompt again. A stale advisory — one on
/// a fixture that no longer contradicts anything — means the reason it records
/// has stopped being true, and a justification that has stopped being true is
/// worse than none, because the next reader believes it. `synth_sounds.py`'s
/// `--verify` pass is the model here: a measured constant with no
/// re-measurement decays into an unmeasured one.
#[test]
fn fixture_intents_agree_with_the_prompt() {
    let fixtures = load_fixtures();
    assert!(!fixtures.is_empty(), "no fixtures loaded");

    let mut missing = Vec::new();
    let mut stale = Vec::new();

    for fix in &fixtures {
        let Some(expected) = fix.expected_intent.as_deref() else {
            // A fixture with no intent expectation cannot contradict the rule.
            assert!(
                fix.intent_advisory.is_none(),
                "{}: carries intent_advisory but asserts no expected_intent",
                fix.id
            );
            continue;
        };

        let contradicts = form_field_length_rule_applies(&fix.raw) && expected != "form_field";

        match (contradicts, fix.intent_advisory.as_deref()) {
            (true, None) => missing.push(format!(
                "{}: raw is {} words with no sentence-ending punctuation, so \
                 POLISH_SYSTEM_PROMPT specifies form_field, but the fixture \
                 expects {}. Either correct expected_intent or add an \
                 intent_advisory naming the rule it collides with.",
                fix.id,
                fix.raw.split_whitespace().count(),
                expected
            )),
            (false, Some(reason)) => stale.push(format!(
                "{}: has an intent_advisory ({:?}) but no longer contradicts the \
                 form_field length rule. Delete it.",
                fix.id, reason
            )),
            _ => {}
        }

        if let Some(reason) = fix.intent_advisory.as_deref() {
            assert!(
                reason.len() > 20,
                "{}: intent_advisory must say which rules collide, got {:?}",
                fix.id,
                reason
            );
        }
    }

    assert!(
        missing.is_empty() && stale.is_empty(),
        "fixture intents disagree with POLISH_SYSTEM_PROMPT:\n  {}\n  {}",
        missing.join("\n  "),
        stale.join("\n  ")
    );
}

/// The advisory set is a decision with a size, so the size is asserted.
///
/// Twelve fixtures collide with the `raw_prompt` rule because the prompt states
/// no precedence between "short utterance (under 8 words)" and "imperative
/// verb, question, or instruction-shaped". That is a gap in
/// `POLISH_SYSTEM_PROMPT` (`transcription/polish.rs`), not in these fixtures,
/// and it is routed there rather than papered over here.
///
/// If someone adds a precedence line to the prompt, this count should drop and
/// this test should fail — which is the point. A pile of advisories that grows
/// without anyone noticing is how "we know about that" becomes "nobody knows
/// about that".
#[test]
fn the_prompts_intent_rules_still_overlap_on_twelve_fixtures() {
    let fixtures = load_fixtures();
    let advisories: Vec<&str> = fixtures
        .iter()
        .filter(|f| f.intent_advisory.is_some())
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(
        advisories.len(),
        12,
        "advisory count changed: {:?}. If POLISH_SYSTEM_PROMPT now states \
         precedence between the form_field length rule and the raw_prompt \
         imperative/question rule, correct these fixtures and this number.",
        advisories
    );
}
