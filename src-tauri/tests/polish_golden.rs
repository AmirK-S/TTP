// Integration tests for the Polaris polish pipeline. Hits the live Groq model
// named by `polish::MODEL` and verifies anti-injection + self-correction + intent
// classification behavior against the golden fixture set.
//
// Tests are `#[ignore]`d by default because they require a `GROQ_API_KEY`
// env var and burn ~25 API calls. Run manually:
//
//   GROQ_API_KEY=gsk_... cargo test --test polish_golden -- --ignored --nocapture
//
// In CI, these can be wired into a periodic job (not per-PR) since polish
// behavior shifts with model updates and we want canary alerts.
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

fn check_fixture(fix: &Fixture, polished: &str, intent: &str) -> Result<(), String> {
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
    if let Some(expected_intent) = &fix.expected_intent {
        if intent != expected_intent {
            return Err(format!(
                "expected_intent={} got intent={}",
                expected_intent, intent
            ));
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore]
async fn polish_golden_fixtures() {
    let api_key = std::env::var("GROQ_API_KEY")
        .expect("GROQ_API_KEY env var must be set to run golden tests");
    let fixtures = load_fixtures();
    let mut failures = Vec::new();
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

                match check_fixture(fix, &polish_result.polished, intent_str) {
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
