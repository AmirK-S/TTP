// Groq Whisper transcription API client

use crate::http_client::{retry_guidance, shared as shared_http, RetryGuidance};
use crate::logging::log_error;
use reqwest::multipart::{Form, Part};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::time::sleep;

/// Compute a retry sleep with ±25% jitter from `base_ms`.
///
/// Jitter avoids retry stampedes when many clients hit the same upstream
/// rate-limit at once. Source of randomness is the low bits of the current
/// monotonic clock — perfectly fine for jitter (we don't need crypto-grade
/// entropy and pulling in `rand` for this would be overkill).
///
/// Only used where the server told us nothing (5xx, transport). When Groq
/// names a delay we sleep exactly that: smearing a stated time ±25% either
/// wakes us early into the same closed window or wastes the user's seconds.
fn jittered_backoff_ms(base_ms: u64) -> u64 {
    // Take ~10 bits of entropy from the nanosecond portion of the clock.
    let entropy = Instant::now().elapsed().subsec_nanos() ^ base_ms as u32;
    // Map to [0, 1) then to [-0.5, 0.5), then scale to ±25% of base.
    let frac = (entropy & 0x3FF) as f32 / 1024.0; // [0, 1)
    let signed = frac - 0.5;                       // [-0.5, 0.5)
    let delta = signed * 0.5 * base_ms as f32;     // ±25% of base
    (base_ms as f32 + delta).max(1.0) as u64
}

/// Groq transcription API endpoint (uses whisper-large-v3)
const GROQ_TRANSCRIPTION_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Maximum number of retry attempts
const MAX_RETRIES: u32 = 3;

/// Base request timeout in seconds (scales up with file size)
const BASE_TIMEOUT_SECS: u64 = 30;

/// Total time transcription may spend *waiting* on rate limits within one
/// dictation, in milliseconds. A budget for the whole call, not a per-sleep
/// cap: two waits of four seconds cost the user the same eight as one.
///
/// This is deliberately more than four times the polish budget
/// (`polish.rs::RETRY_BUDGET_MS` = 2000), and the difference is not taste —
/// it is the fallback. Polish that gives up costs punctuation:
/// `pipeline.rs` pastes the unpolished text on any polish error. Transcription
/// that gives up costs the dictation: the pipeline aborts with
/// `whisper_error` and the user's words are gone. So the two calls are
/// answering different questions. Polish asks "is waiting still faster than
/// the fallback?"; transcription has no fallback and asks "is waiting still
/// cheaper than making the user say it all again?".
///
/// Measured 2026-09-06 over the harvest corpus — 552 transcription requests
/// in `ttp-trace.log`/`.log.1` — plus the 55 rate-limit replies Groq sent on
/// the chat endpoint on 2026-09-02 (`ttp.log`):
///
///   * A successful Whisper call: p50 465ms, p95 1047ms, p99 1589ms,
///     max 2018ms over 550 responses. The call is not the expensive part.
///   * End to end (`dictation.start` → `dictation.finish`): p50 1272ms,
///     p90 2219ms, p99 5276ms.
///   * The delays Groq asks for when it rate-limits this account on the free
///     tier: 0.21s to 8.34s, median 4.08s across 55 replies. 8500ms covers
///     every one of them — the whole measured distribution, not half of it.
///   * What the user pays instead if we refuse: the median dictation on file
///     is 8.73s of speech (n=584, p90 28.0s). Re-recording it costs that
///     8.73s plus the 1272ms pipeline plus recomposing the sentence — and it
///     runs straight back into a rate-limit window that has not reopened.
///
/// So the budget sits in the gap between the longest delay Groq was ever
/// observed to ask for (8.34s) and the cheapest possible re-dictation
/// (8.73s). Waiting is never the worse trade inside that gap. The user's
/// worst case becomes roughly 0.8s of local work + a ~60ms rejection +
/// 8.34s of waiting + a 465ms retry ≈ 9.7s to a correct paste, against
/// ≥10s and a second rate limit for the dictation they would have to redo.
///
/// The old code spent ~1.5s of jittered local backoff across all three
/// attempts and ignored what the server said, so four retries in five were
/// sent before the window they were waiting for had reopened.
///
/// Re-measure when the tier changes: the free-tier limit is what sets the
/// delays Groq asks for, and a paid tier would shrink both sides of this.
const RETRY_BUDGET_MS: u64 = 8500;

/// What to wait on a 429 that names no delay anywhere — median of the 55
/// delays Groq actually asked this account for (4.08s, rounded up).
///
/// Polish refuses to guess here and degrades to unpolished text
/// (`polish.rs` → `no_guidance`). Transcription cannot: the choice is
/// between one unmeasured wait and certainly losing the dictation. Against
/// the measured distribution the median is the wait that clears the window
/// about half the time, and half a dictation saved beats none.
///
/// It is used AT MOST ONCE per dictation (only when nothing has been slept
/// yet), so a server that keeps refusing without explanation costs 4.1s, not
/// 12. Every such attempt is traced with `"blind": true` — the corpus holds
/// **zero** Whisper 429s in 552 requests, so this constant is chat-endpoint
/// evidence applied to the audio endpoint, and that field is how we find out
/// whether it was right.
const BLIND_429_MS: u64 = 4100;

/// What to do after a failed transcription attempt. Pure decision — no clock,
/// no network — so the whole policy is unit-testable without calling Groq.
/// The burst that produced this workstream cost ~54 live calls.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RetryPlan {
    /// Sleep exactly this long. `blind` marks a wait we invented because the
    /// server named no delay, so the trace never conflates a wait Groq asked
    /// for with one we guessed.
    Wait { ms: u64, blind: bool },
    /// Local backoff for a failure the server gave no guidance about
    /// (5xx, transport). Caller jitters this to avoid a retry stampede.
    Backoff { base_ms: u64 },
    /// Stop. The pipeline aborts the dictation with `whisper_error`, so this
    /// is the expensive branch and every arm of it names itself. `reason` is
    /// a trace slug, never user-facing prose.
    GiveUp { reason: &'static str },
}

/// Decide what happens after a failed transcription attempt.
///
/// `status` is `None` for a transport failure (timeout, DNS, connection
/// reset). `attempts_left` is how many attempts remain after this one.
/// `guidance_ms` is what the server asked for, if it said anything.
/// `waited_ms` is what this transcription call has already slept.
fn plan_retry(
    status: Option<u16>,
    attempts_left: u32,
    guidance_ms: Option<u64>,
    waited_ms: u64,
) -> RetryPlan {
    // A 4xx that is not a rate limit is terminal. Every non-429 client error
    // in the log — nine of them, all 403 "Access denied" — is a key or a
    // network block, and no amount of waiting fixes either.
    if let Some(code) = status {
        if (400..500).contains(&code) && code != 429 {
            return RetryPlan::GiveUp {
                reason: "client_error",
            };
        }
    }

    // Rate limits are decided on the server's own guidance, before the
    // attempt count, so the trace names the real cause: "the wait was longer
    // than a dictation can afford" is a different finding from "we ran out of
    // tries", and only the first says the tier is the problem.
    if status == Some(429) {
        let Some(delay_ms) = guidance_ms else {
            // No delay named. One guess, from the measured median, and only
            // before we have slept at all — see `BLIND_429_MS`.
            if waited_ms > 0 {
                return RetryPlan::GiveUp {
                    reason: "no_guidance",
                };
            }
            if attempts_left == 0 {
                return RetryPlan::GiveUp {
                    reason: "attempts_exhausted",
                };
            }
            if BLIND_429_MS > RETRY_BUDGET_MS {
                return RetryPlan::GiveUp {
                    reason: "over_budget",
                };
            }
            return RetryPlan::Wait {
                ms: BLIND_429_MS,
                blind: true,
            };
        };
        if waited_ms.saturating_add(delay_ms) > RETRY_BUDGET_MS {
            return RetryPlan::GiveUp {
                reason: "over_budget",
            };
        }
        if attempts_left == 0 {
            return RetryPlan::GiveUp {
                reason: "attempts_exhausted",
            };
        }
        return RetryPlan::Wait {
            ms: delay_ms,
            blind: false,
        };
    }

    if attempts_left == 0 {
        return RetryPlan::GiveUp {
            reason: "attempts_exhausted",
        };
    }

    // 5xx and transport failures: nothing told us when to come back, so the
    // old local schedule stands (~500ms, ~1000ms). The corpus has zero 5xx in
    // 552 requests and exactly one transport outage (2026-09-05 20:28:54 →
    // 20:29:06, all three attempts refused at the socket), so there is no
    // distribution to fit — unlike the 429 path, which had 55 measurements
    // sitting in the log the whole time. Waiting longer would not have saved
    // that dictation; the network was down for the whole 29s.
    let attempt_index = MAX_RETRIES.saturating_sub(attempts_left);
    RetryPlan::Backoff {
        base_ms: 500 * attempt_index as u64,
    }
}

/// Put the retry decision on a `whisper.attempt` event.
///
/// `whisper.rs` emitted nothing at all before this: `whisper.request` and
/// `whisper.response` are written by the CALLER in `pipeline.rs`, so three
/// attempts burned against a server asking for five seconds appeared in the
/// trace as one slightly slow response. These fields answer what the repeat
/// could not — what the server asked for, where it said so, and whether we
/// honoured it, guessed, or stopped. All values are slugs, booleans and
/// numbers: the app is bilingual and prose belongs in the i18n catalogue.
fn annotate_attempt(
    fields: &mut serde_json::Value,
    plan: &RetryPlan,
    guidance: Option<&RetryGuidance>,
) {
    let Some(obj) = fields.as_object_mut() else {
        return;
    };
    if let Some(g) = guidance {
        obj.insert("retry_after_ms".to_string(), g.delay_ms.into());
        obj.insert("retry_source".to_string(), g.source.into());
    }
    match plan {
        RetryPlan::GiveUp { reason } => {
            obj.insert("decision".to_string(), "give_up".into());
            obj.insert("reason".to_string(), (*reason).into());
        }
        RetryPlan::Wait { blind, .. } => {
            obj.insert("decision".to_string(), "retry".into());
            // Only ever true on a 429 with no stated delay. If this field
            // shows up in the corpus, `BLIND_429_MS` has real evidence to be
            // re-derived from and should stop being a borrowed number.
            if *blind {
                obj.insert("blind".to_string(), true.into());
            }
        }
        RetryPlan::Backoff { .. } => {
            obj.insert("decision".to_string(), "retry".into());
        }
    }
}

/// Transcribe audio file using Groq (whisper-large-v3 model)
///
/// Retries on rate limits using the delay Groq itself names, within
/// [`RETRY_BUDGET_MS`]; falls back to local backoff only for failures the
/// server gave no guidance about.
///
/// # Arguments
/// * `api_key` - Groq API key
/// * `audio_path` - Path to the audio file (WAV format)
/// * `prompt` - Optional dictionary biasing prompt (proper nouns).
/// * `language` - ISO-639-1 language hint ("en", "fr"). Passing `None` lets
///   Whisper auto-detect — known to pick zh/ru/ko on silence/noise, which
///   then surfaces as "spurious Chinese" transcription. Always pass the
///   user's UI language here when we know it.
///
/// # Returns
/// * `Ok(String)` - Transcription text on success
/// * `Err(String)` - Error message on failure
pub async fn transcribe_audio(
    api_key: &str,
    audio_path: &str,
    prompt: Option<&str>,
    language: Option<&str>,
) -> Result<String, String> {
    transcribe_with_provider(
        api_key,
        audio_path,
        GROQ_TRANSCRIPTION_URL,
        "whisper-large-v3",
        "Groq",
        prompt,
        language,
    )
    .await
}

/// Internal function to transcribe audio with a specific provider.
///
/// Retry policy lives entirely in [`plan_retry`]; this function only executes
/// it and traces the result.
async fn transcribe_with_provider(
    api_key: &str,
    audio_path: &str,
    transcription_url: &str,
    model: &str,
    _provider_name: &str,
    prompt: Option<&str>,
    language: Option<&str>,
) -> Result<String, String> {
    // Convert model to owned String for Form::text (requires 'static)
    let model = model.to_string();

    // Read audio file bytes
    let audio_bytes = fs::read(audio_path)
        .await
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Get filename and MIME type from path for the multipart form
    let filename = Path::new(audio_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.wav")
        .to_string();

    let mime_type = "audio/wav";

    // Scale timeout based on file size: base + 2s per MB.
    // Applied per-request below so the shared client (which has no global
    // timeout) can be reused across calls of different sizes.
    let file_mb = audio_bytes.len() as u64 / (1024 * 1024);
    let timeout_secs = BASE_TIMEOUT_SECS + file_mb * 2;

    // Reuse the process-wide HTTP client so we keep TCP/TLS connections warm
    // across whisper -> polish -> next-recording calls.
    let client = shared_http();

    let mut last_error = String::new();
    // `waited_total_ms` is what this call has already slept and is what
    // `RETRY_BUDGET_MS` is spent against. `waited_before_attempt_ms` is the
    // sleep that preceded the attempt now in flight: it goes on the trace so
    // an attempt that honoured Groq's own delay is distinguishable from one
    // that fired blind — which nothing in the old trace was.
    let mut waited_total_ms: u64 = 0;
    let mut waited_before_attempt_ms: u64 = 0;
    for attempt in 0..MAX_RETRIES {
        let attempts_left = MAX_RETRIES - attempt - 1;

        // Per-attempt timing. `whisper.response` in `pipeline.rs` measures the
        // whole call, so it cannot tell one slow transcription from a failure
        // plus a retry — and those need opposite fixes.
        let attempt_started = Instant::now();
        let attempt_ms = || attempt_started.elapsed().as_millis() as u64;

        // Build multipart form - need to recreate each attempt since Part consumes bytes
        let file_part = Part::bytes(audio_bytes.clone())
            .file_name(filename.clone())
            .mime_str(mime_type)
            .map_err(|e| format!("Failed to set MIME type: {}", e))?;

        let mut form = Form::new()
            .text("model", model.clone())
            .text("response_format", "text")
            .text("temperature", "0")
            .part("file", file_part);

        if let Some(prompt_value) = prompt {
            form = form.text("prompt", prompt_value.to_string());
        }

        // Pin the decoder to a specific language. Whisper's auto-detect
        // routinely picks zh/ru/ko on silence + noise frames — the user
        // dictates 2 seconds in English and the model returns a Chinese
        // sentence. Forcing the language eliminates that failure mode and
        // is also faster (skips the detect pass).
        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        // Make the request. Per-request timeout scales with audio size and
        // is applied here (not on the shared client) so other call sites
        // keep their own timeouts.
        match client
            .post(transcription_url)
            .timeout(Duration::from_secs(timeout_secs))
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    // Parse response text
                    let text = response
                        .text()
                        .await
                        .map(|text| text.trim().to_string())
                        .map_err(|e| format!("Failed to read transcription response: {}", e))?;
                    crate::trace::event(
                        "whisper.attempt",
                        serde_json::json!({
                            "n": attempt + 1,
                            "status": 200,
                            "ms": attempt_ms(),
                            // Non-zero here is the dictation that an honoured
                            // rate-limit wait saved. Zero on every one of the
                            // 552 requests in the corpus so far.
                            "waited_ms": waited_before_attempt_ms,
                        }),
                    );
                    return Ok(text);
                } else {
                    // Headers before the body: `text()` consumes the response,
                    // and `retry-after` lives in the headers.
                    let headers = response.headers().clone();
                    let error_body = response.text().await.unwrap_or_default();
                    let status_code = status.as_u16();
                    // Keep the status code verbatim in the message — the
                    // pipeline reads it to tell 429/401/403 apart and pick a
                    // friendlier message.
                    last_error = format!("Transcription API error: {} - {}", status, error_body);
                    log_error(&format!(
                        "API error {}: {}",
                        status,
                        &error_body[..error_body.len().min(300)]
                    ));

                    let guidance = retry_guidance(&headers, &error_body);
                    let plan = plan_retry(
                        Some(status_code),
                        attempts_left,
                        guidance.as_ref().map(|g| g.delay_ms),
                        waited_total_ms,
                    );

                    let mut fields = serde_json::json!({
                        "n": attempt + 1,
                        "status": status_code,
                        "ms": attempt_ms(),
                        "waited_ms": waited_before_attempt_ms,
                    });
                    annotate_attempt(&mut fields, &plan, guidance.as_ref());
                    crate::trace::event("whisper.attempt", fields);

                    match execute(&plan) {
                        None => return Err(last_error),
                        Some(ms) => {
                            waited_before_attempt_ms = ms;
                            waited_total_ms += ms;
                            sleep(Duration::from_millis(ms)).await;
                        }
                    }
                }
            }
            Err(e) => {
                // Transport failure (timeout, DNS, connection reset).
                last_error = format!("Transcription request failed: {}", e);
                log_error(&format!("Network error: {}", e));

                // No status, so no server guidance: local backoff or nothing.
                let plan = plan_retry(None, attempts_left, None, waited_total_ms);

                let mut fields = serde_json::json!({
                    "n": attempt + 1,
                    // A timeout lands here after `timeout_secs`, so `ms` near
                    // that value is a hung upload rather than a refused one.
                    "error": e.to_string(),
                    "timed_out": e.is_timeout(),
                    "ms": attempt_ms(),
                    "timeout_secs": timeout_secs,
                    "waited_ms": waited_before_attempt_ms,
                });
                annotate_attempt(&mut fields, &plan, None);
                crate::trace::event("whisper.attempt", fields);

                match execute(&plan) {
                    None => return Err(last_error),
                    Some(ms) => {
                        waited_before_attempt_ms = ms;
                        waited_total_ms += ms;
                        sleep(Duration::from_millis(ms)).await;
                    }
                }
            }
        }
    }

    // All retries exhausted
    Err(last_error)
}

/// Turn a plan into the number of milliseconds to sleep, or `None` to stop.
/// Split out so the two call sites cannot drift apart, and so the jitter —
/// the one non-deterministic step — has exactly one home.
fn execute(plan: &RetryPlan) -> Option<u64> {
    match plan {
        RetryPlan::GiveUp { .. } => None,
        RetryPlan::Wait { ms, .. } => Some(*ms),
        RetryPlan::Backoff { base_ms } => Some(jittered_backoff_ms(*base_ms)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- retry policy -------------------------------------------------
    //
    // The whole policy is `plan_retry`, and it is a pure function: no clock,
    // no network, no Groq call. An earlier agent proved this loop by calling
    // the live API; it cost ~54 calls, exhausted the tier and contaminated
    // the maintainer's own trace log. Everything below runs offline.

    /// The delays Groq actually asked this account for, in seconds, across
    /// the 55 rate-limit replies in `ttp.log` on 2026-09-02. Sorted.
    ///
    /// Measured on the chat endpoint — the corpus contains zero Whisper 429s
    /// in 552 requests — but it is the same organization, tier and limiter,
    /// and it is the only measurement of Groq's asks that exists.
    const OBSERVED_DELAYS_S: &[f64] = &[
        0.21, 0.46, 0.58, 0.64, 0.66, 0.66, 0.67, 0.70, 0.93, 1.18, 1.19,
        1.59, 1.62, 1.91, 2.15, 2.18, 2.29, 2.44, 2.45, 2.53, 2.54, 2.66,
        3.05, 3.32, 3.83, 3.88, 4.00, 4.08, 4.12, 4.34, 4.52, 4.64, 4.82,
        4.93, 5.14, 5.35, 5.39, 5.48, 5.68, 5.78, 5.87, 5.96, 6.09, 6.20,
        6.35, 6.57, 7.05, 7.19, 7.33, 7.51, 7.55, 7.55, 7.56, 7.70, 8.34,
    ];

    /// Median length of a dictation in the harvest corpus, in milliseconds
    /// (584 `audio.duration` events, p50 8.73s). This is the floor cost of
    /// the alternative to waiting: saying the whole thing again.
    const MEDIAN_DICTATION_MS: u64 = 8730;

    #[test]
    fn a_rate_limit_waits_exactly_what_the_server_asked_for() {
        // The finding: Groq said 6.57s and the code slept ~500ms. Anything
        // inside the budget is now honoured to the millisecond — no jitter,
        // no invented multiple of 500.
        assert_eq!(
            plan_retry(Some(429), 2, Some(6570), 0),
            RetryPlan::Wait { ms: 6570, blind: false }
        );
        assert_eq!(
            plan_retry(Some(429), 2, Some(578), 0),
            RetryPlan::Wait { ms: 578, blind: false }
        );
    }

    #[test]
    fn the_budget_covers_every_delay_groq_was_ever_observed_to_ask_for() {
        // This is the whole difference from polish, whose 2000ms budget
        // refuses 41 of these 55 because it can fall back to unpolished text.
        // Transcription cannot fall back to anything, so it honours all 55.
        let refused: Vec<u64> = OBSERVED_DELAYS_S
            .iter()
            .map(|s| (s * 1000.0).round() as u64)
            .filter(|ms| !matches!(plan_retry(Some(429), 2, Some(*ms), 0), RetryPlan::Wait { .. }))
            .collect();
        assert!(refused.is_empty(), "refused delays Groq asked for: {:?}", refused);
        // And the old backoff, at ~1.5s spent across all three attempts,
        // would have covered only 11 of the 55 even spent in one go.
        assert_eq!(OBSERVED_DELAYS_S.iter().filter(|s| **s <= 1.5).count(), 11);
    }

    #[test]
    fn waiting_is_never_more_expensive_than_re_dictating() {
        // The budget's justification in one assertion: the most we will ever
        // make someone wait is still less than the median dictation they
        // would otherwise have to record again — before counting the pipeline
        // they would pay a second time, and the rate-limit window that has
        // not reopened for them either.
        assert!(
            RETRY_BUDGET_MS < MEDIAN_DICTATION_MS,
            "budget {}ms exceeds the {}ms cost of just saying it again",
            RETRY_BUDGET_MS,
            MEDIAN_DICTATION_MS
        );
        // And it clears the longest delay on file, or it would refuse asks it
        // could afford.
        let longest_ms = (OBSERVED_DELAYS_S.last().unwrap() * 1000.0).round() as u64;
        assert!(RETRY_BUDGET_MS >= longest_ms, "budget below the longest observed ask");
    }

    #[test]
    fn a_rate_limit_longer_than_the_budget_gives_up_rather_than_freezing() {
        // A tier change or a hostile server must not turn a dictation into a
        // minute-long hang. Above the budget we stop, and the trace says why.
        assert_eq!(
            plan_retry(Some(429), 2, Some(30_000), 0),
            RetryPlan::GiveUp { reason: "over_budget" }
        );
    }

    #[test]
    fn the_budget_is_spent_across_the_whole_call_not_per_sleep() {
        // Two 5s waits are as bad as one 10s wait to the person waiting.
        assert_eq!(
            plan_retry(Some(429), 1, Some(5000), 5000),
            RetryPlan::GiveUp { reason: "over_budget" }
        );
        // Exactly on the budget still runs: the cap is a ceiling, not a fence.
        assert_eq!(
            plan_retry(Some(429), 1, Some(4250), 4250),
            RetryPlan::Wait { ms: 4250, blind: false }
        );
    }

    #[test]
    fn no_plan_ever_sleeps_longer_than_the_budget() {
        // Belt and braces against a broken or hostile Retry-After: a server
        // asking for an hour must never freeze a dictation.
        for guidance in [Some(0u64), Some(1), Some(4_100), Some(8_500), Some(8_501),
                         Some(60_000), Some(3_600_000), Some(u64::MAX), None] {
            for waited in [0u64, 500, 4_100, 8_500] {
                if let RetryPlan::Wait { ms, .. } =
                    plan_retry(Some(429), 2, guidance, waited)
                {
                    assert!(
                        waited + ms <= RETRY_BUDGET_MS,
                        "guidance {:?} after {}ms waited slept {}ms",
                        guidance,
                        waited,
                        ms
                    );
                }
            }
        }
    }

    #[test]
    fn a_rate_limit_with_no_guidance_guesses_once_from_the_measured_median() {
        // Polish refuses to guess because it can paste unpolished text.
        // Transcription's alternative is losing the dictation, so it spends
        // one median-length wait — and marks it blind so the trace never
        // passes it off as something Groq asked for.
        assert_eq!(
            plan_retry(Some(429), 2, None, 0),
            RetryPlan::Wait { ms: BLIND_429_MS, blind: true }
        );
        // The guess is the median of the 55 measured asks, not a round number
        // someone liked. Half of them are shorter, so it clears the window
        // about half the time.
        let mut ms: Vec<u64> = OBSERVED_DELAYS_S.iter().map(|s| (s * 1000.0).round() as u64).collect();
        ms.sort_unstable();
        let median = ms[ms.len() / 2];
        assert_eq!(BLIND_429_MS, 4100);
        assert!(
            BLIND_429_MS.abs_diff(median) <= 50,
            "blind wait {}ms drifted from the measured median {}ms",
            BLIND_429_MS,
            median
        );
    }

    #[test]
    fn a_blind_guess_is_never_made_twice_in_one_dictation() {
        // One unmeasured wait is a trade. Three is the hammering this
        // workstream exists to stop: 54 calls in 20 seconds.
        assert_eq!(
            plan_retry(Some(429), 1, None, BLIND_429_MS),
            RetryPlan::GiveUp { reason: "no_guidance" }
        );
        // Even a 1ms local backoff counts as "we have already slept".
        assert_eq!(
            plan_retry(Some(429), 1, None, 1),
            RetryPlan::GiveUp { reason: "no_guidance" }
        );
    }

    #[test]
    fn a_rate_limit_on_the_last_attempt_says_so() {
        assert_eq!(
            plan_retry(Some(429), 0, Some(500), 0),
            RetryPlan::GiveUp { reason: "attempts_exhausted" }
        );
        assert_eq!(
            plan_retry(Some(429), 0, None, 0),
            RetryPlan::GiveUp { reason: "attempts_exhausted" }
        );
    }

    #[test]
    fn over_budget_outranks_attempts_exhausted() {
        // Both are true on the last attempt; only one names the tier as the
        // cause, and that is the one worth reading in the trace.
        assert_eq!(
            plan_retry(Some(429), 0, Some(30_000), 0),
            RetryPlan::GiveUp { reason: "over_budget" }
        );
    }

    #[test]
    fn non_rate_limit_client_errors_are_terminal() {
        // Every non-429 client error in `ttp.log` is a 403 "Access denied"
        // (nine of them, one inside the trace corpus as dictation 0084-e858,
        // aborted in 863ms). Retrying a key or a network block only adds
        // latency to a dictation that is already lost.
        for code in [400, 401, 403, 404, 413, 422] {
            assert_eq!(
                plan_retry(Some(code), 2, None, 0),
                RetryPlan::GiveUp { reason: "client_error" },
                "status {}",
                code
            );
        }
        // Even if such a response carried a Retry-After, it is still terminal.
        assert_eq!(
            plan_retry(Some(403), 2, Some(100), 0),
            RetryPlan::GiveUp { reason: "client_error" }
        );
    }

    #[test]
    fn server_errors_and_transport_failures_keep_the_local_backoff() {
        // Nothing told us when to come back, so the old schedule stands:
        // ~500ms after the first failure, ~1000ms after the second. Zero 5xx
        // in 552 requests, and the one transport outage on file refused all
        // three attempts at the socket in 29s — no wait would have helped.
        assert_eq!(plan_retry(Some(500), 2, None, 0), RetryPlan::Backoff { base_ms: 500 });
        assert_eq!(plan_retry(Some(503), 1, None, 0), RetryPlan::Backoff { base_ms: 1000 });
        assert_eq!(plan_retry(None, 2, None, 0), RetryPlan::Backoff { base_ms: 500 });
        assert_eq!(plan_retry(None, 1, None, 0), RetryPlan::Backoff { base_ms: 1000 });
        assert_eq!(
            plan_retry(None, 0, None, 0),
            RetryPlan::GiveUp { reason: "attempts_exhausted" }
        );
    }

    #[test]
    fn the_whole_loop_costs_at_most_the_budget_plus_the_local_backoff() {
        // Walk the three attempts as the loop would, feeding each decision
        // back in. Whatever Groq asks for, the sleeping stops somewhere.
        for delay in [0u64, 500, 4_080, 8_340, 8_501, 60_000] {
            let mut waited = 0u64;
            for attempt in 0..MAX_RETRIES {
                let attempts_left = MAX_RETRIES - attempt - 1;
                match plan_retry(Some(429), attempts_left, Some(delay), waited) {
                    RetryPlan::Wait { ms, .. } => waited += ms,
                    RetryPlan::Backoff { base_ms } => waited += base_ms,
                    RetryPlan::GiveUp { .. } => break,
                }
            }
            assert!(
                waited <= RETRY_BUDGET_MS,
                "delay {}ms produced {}ms of total sleep",
                delay,
                waited
            );
        }
    }

    // ---- trace ---------------------------------------------------------

    #[test]
    fn the_trace_tells_the_four_cases_apart() {
        // Honoured wait, blind guess, refusal, transport backoff. In the old
        // code all four were the same thing in the log: nothing at all.
        let guidance = RetryGuidance {
            delay_ms: 6353,
            source: crate::http_client::SOURCE_BODY,
        };
        let mut retried = serde_json::json!({ "status": 429 });
        annotate_attempt(&mut retried, &RetryPlan::Wait { ms: 6353, blind: false }, Some(&guidance));
        assert_eq!(retried["decision"], "retry");
        assert_eq!(retried["retry_after_ms"], 6353);
        assert_eq!(retried["retry_source"], "body");
        assert!(retried.get("blind").is_none(), "an honoured delay is not a guess");
        assert!(retried.get("reason").is_none());

        let mut blind = serde_json::json!({ "status": 429 });
        annotate_attempt(&mut blind, &RetryPlan::Wait { ms: BLIND_429_MS, blind: true }, None);
        assert_eq!(blind["decision"], "retry");
        assert_eq!(blind["blind"], true);
        assert!(blind.get("retry_after_ms").is_none(), "we invented this one");

        let long = RetryGuidance {
            delay_ms: 30_000,
            source: crate::http_client::SOURCE_HEADER,
        };
        let mut gave_up = serde_json::json!({ "status": 429 });
        annotate_attempt(&mut gave_up, &RetryPlan::GiveUp { reason: "over_budget" }, Some(&long));
        assert_eq!(gave_up["decision"], "give_up");
        assert_eq!(gave_up["reason"], "over_budget");
        assert_eq!(gave_up["retry_after_ms"], 30_000);
        assert_eq!(gave_up["retry_source"], "retry_after");

        let mut outage = serde_json::json!({ "timed_out": true });
        annotate_attempt(&mut outage, &RetryPlan::Backoff { base_ms: 500 }, None);
        assert_eq!(outage["decision"], "retry");
        assert!(outage.get("retry_after_ms").is_none());
        assert!(outage.get("blind").is_none());
    }

    #[test]
    fn trace_fields_are_slugs_booleans_and_numbers_never_prose() {
        // The app is bilingual; `npm run i18n:check` polices the UI strings
        // and nothing polices Rust, so this does.
        for plan in [
            RetryPlan::GiveUp { reason: "over_budget" },
            RetryPlan::GiveUp { reason: "no_guidance" },
            RetryPlan::GiveUp { reason: "attempts_exhausted" },
            RetryPlan::GiveUp { reason: "client_error" },
        ] {
            let mut fields = serde_json::json!({});
            annotate_attempt(&mut fields, &plan, None);
            let reason = fields["reason"].as_str().unwrap();
            assert!(
                reason.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !reason.contains(' '),
                "{:?} is not a slug",
                reason
            );
        }
    }

    #[test]
    fn every_give_up_reason_is_reachable_from_a_real_response() {
        // A reason no response can produce is a slug that will never appear
        // in the corpus, which is worse than no slug at all.
        use std::collections::HashSet;
        let mut seen: HashSet<&'static str> = HashSet::new();
        for status in [None, Some(403), Some(429), Some(500)] {
            for attempts_left in [0u32, 1, 2] {
                for guidance in [None, Some(0u64), Some(4_100), Some(30_000)] {
                    for waited in [0u64, 4_100, 8_500] {
                        if let RetryPlan::GiveUp { reason } =
                            plan_retry(status, attempts_left, guidance, waited)
                        {
                            seen.insert(reason);
                        }
                    }
                }
            }
        }
        for expected in ["client_error", "no_guidance", "over_budget", "attempts_exhausted"] {
            assert!(seen.contains(expected), "{} is unreachable", expected);
        }
    }

    // ---- execution -----------------------------------------------------

    #[test]
    fn execute_sleeps_what_the_plan_says_and_stops_on_give_up() {
        assert_eq!(execute(&RetryPlan::Wait { ms: 6353, blind: false }), Some(6353));
        assert_eq!(execute(&RetryPlan::Wait { ms: 0, blind: false }), Some(0));
        assert_eq!(execute(&RetryPlan::GiveUp { reason: "over_budget" }), None);
        // Backoff is the only jittered branch, and it stays within ±25%.
        let ms = execute(&RetryPlan::Backoff { base_ms: 500 }).unwrap();
        assert!((375..=625).contains(&ms), "jittered backoff out of range: {}", ms);
    }

    #[test]
    fn jitter_never_returns_zero() {
        // A zero sleep is an immediate retry, which is the defect in a
        // different costume.
        for base in [0u64, 1, 500, 1000, 1500] {
            assert!(jittered_backoff_ms(base) >= 1, "base {} slept 0ms", base);
        }
    }
}
