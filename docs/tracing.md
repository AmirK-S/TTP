# Reading a dictation trace

`ttp-trace.log` records what happened to every dictation, stage by stage. It
exists to answer one question that the ordinary log cannot: **"I pressed, I
spoke, and nothing was written — where did my words go?"**

That failure is silent by construction. Seven different stages can legitimately
end a dictation with no text, and from the user's seat all seven look identical:
an empty target app. The trace makes each one leave a line.

## Where it lives

```
~/Library/Application Support/com.ttp.desktop/ttp-trace.log     # macOS
%APPDATA%\com.ttp.desktop\ttp-trace.log                         # Windows
```

Settings → Advanced → Diagnostics → **Show log folder** opens it. Rotates at
2.5 MB, keeping `ttp-trace.log.1` … `.3`.

It is also readable from inside the app: `docs/trace-api.md` is the command
surface a viewer UI consumes, and it returns the same records this file
describes.

Unlike `ttp.log`, the trace is **not** filtered by log level. Release builds
default to `warn`, which is precisely why every silent-drop path used to be
invisible; gating the trace behind that threshold would reproduce the blind
spot it was written to remove.

## Line format

```
[2026-08-26 08:48:57.412] [0007-3f2a] +    0ms dictation.start  {"kind":"recording","verbose":false,"dur_ms":0}
[2026-08-26 08:48:57.418] [0007-3f2a] +    6ms audio.duration   {"secs":7.52,"wav_bytes":481324,"dur_ms":6}
[2026-08-26 08:48:58.902] [0007-3f2a] + 1490ms whisper.response {"chars":87,"sha8":"9f2c1ab0","attempt":1,"ms":1484,"dur_ms":1484}
[2026-08-26 08:48:59.118] [0007-3f2a] + 1706ms paste.verify     {"ax_readable":true,"changed":true,"delta_chars":87,"expected_chars":87,"settled_ms":48}
[2026-08-26 08:48:59.121] [0007-3f2a] + 1709ms dictation.finish {"outcome":"pasted","ms":1709,"chars":87,"words":16,"dur_ms":3}
```

- `[0007-3f2a]` — the trace id. `grep 0007-3f2a ttp-trace.log` gives you that
  one dictation and nothing else. The `0007` is a per-session sequence number,
  so ids sort in the order the dictations happened.
- `+1490ms` — elapsed since the dictation began, so slow stages are obvious.
- `dur_ms` — on **every** stage of a dictation: milliseconds since the previous
  stage. The elapsed column tells you when a stage ended; `dur_ms` tells you
  what it cost, without subtracting by hand. `ms`, where a stage also carries
  it, times one specific call inside that stage — `dur_ms` includes whatever
  happened before the call started, `ms` does not.
- `[········]` in the id column marks a standalone event (a hotkey press, an
  event-tap recovery) rather than a dictation stage.

### Fields that can appear on any line

Three fields are not part of any one stage. They are attached to whatever
record happens to be passing when there is something to report, because the
thing they report is the trace itself being damaged, and a report that needs
its own line would be lost by the same failure.

- `trace_dropped_lines` — the writer's bounded queue overflowed and this many
  records were never handed to it. Carried by the next record that fits.
- `trace_write_failures` — this many records were handed to the writer and the
  *file* rejected them. `append_line` cannot report its own failure by
  logging, so it counts and the next record that lands carries the tally. If
  that record is also lost the tally is put back, so the number is a true
  total rather than a most-recent one. **A trace that goes quiet has to say
  that it did**; these two are how it says so.
- `lock_wait_ms` — on `keychain.slow`. How long this caller blocked behind
  *another* caller's in-flight keychain read. Zero means it did the read
  itself. This is the single-flight proof: a burst of concurrent callers
  should produce one read with `lock_wait_ms:0` and the rest either cached
  (no line at all) or waiting, and never N callers each paying the full
  securityd cost. See "When a dictation takes minutes".

## The stages

| Stage | What it tells you |
|---|---|
| `app.launched` | Session boundary, with version and platform. Everything below it belongs to one run of the app. |
| `hotkey.press` / `hotkey.release` | The Fn/Globe key was seen. **No line here means the input layer never fired** — the recording never started. |
| `hotkey.double_tap` / `hotkey.hands_free_stop` | The two hands-free edges: the double tap that latched recording on, and the press that ended it. |
| `hotkey.tap_armed` / `hotkey.tap_create_failed` / `hotkey.tap_abandoned` | The tap's lifecycle at its ends. `tap_abandoned` is terminal: after three rebuilds the Fn key does nothing until relaunch. |
| `hotkey.event_dropped` | A hotkey event arrived while the state lock was held and was discarded. The press happened; nothing came of it. |
| `state.transition` | Every move between Idle / Recording / Processing. A session parked in Processing makes all later presses silent no-ops. |
| `capture.start_failed` / `capture.stop_failed` | Recording never started, or the finished recording could not be retrieved. Covers all nine early exits in the capture layer. |
| `dictation.rejected` | Audio was captured and then thrown away before transcription — rate limit, or a path that failed validation. |
| `hotkey.tap_rearmed` | macOS had disabled our event tap and we re-armed it. Every Fn press between the disable and this line was lost. A climbing `streak` means re-arming is not working. |
| `hotkey.tap_rebuilt` | Re-arming stopped helping, so the tap was torn down and recreated. `CGEventTapEnable` on a tap the window server has written off is a no-op — only a fresh tap restores the Fn key. |
| `hotkey.stale_fn_cleared` | The Globe key was latched "held" and we forced it down. Keystrokes injected before this were being routed to the Globe shortcut layer. |
| `hotkey.timer_stall` | The 20 ms poll timer skipped `gap_ms`. The process was descheduled — nothing advanced during that window: no hotkey, no state machine, no in-flight dictation. TTP now holds an activity assertion for the whole Recording → Idle window (see `crate::activity`), so a stall spanning a dictation should no longer be possible; one that still appears is worth investigating. |
| `capture.start` | Which microphone actually served the recording, its rate/channels/format, whether it is the OS default, and what the user had asked for. |
| `capture.stop` | Samples the callback delivered, and whether the OS default input changed while the user was talking. |
| `capture.stop_waited_for_start` | The stop path found a start still in flight and waited for it, so the recording is collected rather than orphaned. `ms` is how long it waited; **`timed_out:true` is the interesting one** — it means the 3-second settle window elapsed with the start still unfinished, which the constant's own comment says cannot happen. When it does, `capture_arbiter` is the only thing between the user and a live microphone. |
| `capture.orphan_prevented` | **The arbiter refused a start.** A stream had been built, and by the time it asked to go live the state machine no longer wanted one — `reason:"user_idle"` (the press already concluded; this is the 2026-08-30 shape) or `reason:"superseded_by_newer_press"`. The stream is torn down before this line is written, so the line means the microphone is off. `build_ms` is how long the start took to build; the incident's was 544 ms. |
| `capture.orphan_reclaimed` | **The Idle backstop closed a live capture nobody was coming to collect.** The reverse ordering: the start published in the gap between a stop that found nothing and the transition to Idle. Carries `samples`, the `device`, and `wav_finalised`. The unusable WAV is deleted. As with `orphan_prevented`, the stream is dropped before the line is written. |
| `capture.stale_dropped` | A new `start_recording` found a capture still in `STATE` from a previous cycle and closed it. `samples` says how much it had written. One of these means an earlier cycle ended with the microphone open and neither the stop nor the backstop caught it. |
| `capture.dead_input_detected` | The microphone has delivered nothing but zeros for `ms` past the grace period, **while the user is still talking**. Emitted once per capture. This is `dead_capture` said at second two instead of at the end: told early, the user loses one sentence and goes to fix their headphones. |
| `audio.duration` / `audio.signal` | How much audio, how loud. `avg_rms` below `floor` means the silence gate will drop it; `peak` and `nonzero_ratio` distinguish a quiet room from a dead device. |
| `audio.convert` | Stereo 48 kHz → mono 16 kHz, and the size change. |
| `whisper.request` / `whisper.response` | Bytes sent, language pinned, latency, and how many characters came back. `attempt:2` means the first call returned an empty body. |
| `whisper.retry` | The first call came back with nothing and we are asking once more before surfacing `no_speech`. Carries `reason:"empty_body"`. |
| `polish.decision` | Whether polish was going to be attempted at all, and why: `setting_enabled`, `quota_ok`. Emitted before the attempt, so a dictation with no `polish.attempt` says here whether it was skipped or never eligible. |
| `polish.attempt` | One call to the polish model, with its `model`, `status` and `ms`. A non-200 here is the shape `remote-call-failed` keys off — 403, 429 and 404 look identical to the user and are three different bugs. |
| `polish` | Now also carries `ms`, and the *reason*: `guard_reason` when the guard rejected the model's answer, `error_category` (`rate_limited` / `invalid_api_key` / `polish_failed`) when the call failed. Both used to exist only as a Sentry breadcrumb, which is off by default and is not in the file a user attaches to a bug report. |
| `cleanup`, `polish`, `dictionary` | Each text transformation, with `changed` and before/after character counts. A `to.chars` of 0 names the stage that emptied the transcription. `polish.outcome` is `applied` / `failed` / `guard_rejected` / `skipped` — what actually happened, not whether it was allowed to try. |
| `polish.outage` | Polish has failed `consecutive_failures` times in a row against `model`. Emitted on every failure; the user is notified once per session at three. |
| `paste.accessibility` | `tcc_trusted` vs `ax_probe_ok`. Trusted-but-not-working is the stale-TCC state left behind by in-place app updates. |
| `paste.decision` | `type` (direct keystrokes) or `clipboard` (Cmd+V), and how many characters. |
| `paste.skipped` | Injection was not attempted at all. `reason:"no_accessibility"` — the text went to the clipboard and System Settings was opened for the user. Distinct from `paste.result {"ok":false}`, which means we tried and failed. |
| `paste.modifiers` | A modifier key was still held at injection time. Only emitted when one was. |
| `paste.result` | Whether the events were posted. |
| `paste.verify` | Whether they **landed**. See below. |
| `clipboard.restore` | The user's pre-record clipboard was put back. |
| `correction_window.started` | The dictionary correction watcher was armed. |
| `history.saved`, `usage.recorded`, `files.cleaned` | Post-paste bookkeeping. All trivial and synchronous — a large jump between any two of these means the process stalled, not that the step is slow. |
| `usage.polish_recorded` | The polish usage record was written, **and it was written off the critical path**. That is what the line is for: it is emitted from a `spawn_blocking`, so its timestamp lands *after* `paste.result` on the same dictation. A timestamp that lands before it means the move back onto the path has been undone. `ms` is what the write cost, keychain included. |
| `settings.snapshot` | The configuration this dictation ran under: polish, transcription language, VAD, hands-free, history, diagnostics, companion face. A dictation is a function of its settings and now says which ones. |
| `keychain.api_key` | The Groq key read, **timed**. It is a keychain round-trip sitting between the user's last word and the Whisper call, and it is unbounded — see "When a dictation takes minutes". |
| `keychain.warmed` | The startup pre-warm, per account, with its cost. A large number here is *good news*: the bill was paid on a thread nobody was waiting on. Three accounts warm at launch; `account:"groq_api_key"` is the one that matters, because it is the only one whose read sits between the user's last word and the Whisper call. It also carries `found` — whether a key exists, never the key and never its length. |
| `keychain.slow` | Any keychain call that took more than 50 ms. Emitted only when it did, because a warm read is sub-millisecond and a line per usage record would be noise. |
| `filter.hallucination` | `matched` — **including `false`**. Whisper returned real characters and the filter let them through. |
| `filter.glossary_ghost` | `matched`, plus `considered`: whether the filter was eligible at all (a dictionary exists, the take is short). |
| `filter.prompt_introducer` | Same shape. `considered:false` means the recording was too long or too wordy for the filter to apply. |
| `clipboard.write` | The transcription going onto the clipboard, timed. `ok:false` is followed by an abort — the text is gone from everywhere. |
| `history.saved` | Now also emitted when history is **off**, as `{"skipped":"history_disabled"}`. |
| `correction_window.started` | Carries `armed`. It used to be written unconditionally, including on the path that skipped arming. |
| `vad.armed` / `vad.fired` / `vad.disarmed` | The auto-stop watchdog: when it started, whether it cut the recording, and whether the stop that ended it was its own or the user's. |
| `hotkey.tap_health` | **The event tap is alive.** Every five minutes while healthy, and immediately after a recovery. The absence of `hotkey.tap_*` lines used to be ambiguous between "fine" and "not running"; this settles it and bounds any outage to five minutes. |
| `companion.face` / `companion.named` / `companion.pill_hidden` / `companion.state` | The Companion survival test — see `docs/companion-faces-design.md` §2. Booleans, lengths and day counts; never the name. |
| `permission.tcc_reset` | **We are about to destroy the user's granted Accessibility permission.** `tccutil reset` is run when a stale-TCC state is detected, and until Polaris it left one `log_warn` and no trace line at all — so a user who was suddenly re-prompted had nothing explaining why. Emitted *before* the command runs, from the one function that runs it, carrying the two probe values that justified the decision (`api_trusted`, `ax_probe_ok`) plus the `bundle_id` and `version`. |
| `permission.tcc_reset_result` | What `tccutil` said. `ok:true`, or `ok:false` with `stderr` / `error`. The grant is gone either way; this separates "reset and re-prompted" from "asked to reset and was refused". |
| `permission.notify` / `permission.notify_failed` | The UI was told a permission is missing. Both fire during Tauri `setup()`, when the webview may not have mounted, so the banner can be emitted to nobody. `notify` with `emitted:true` is not proof a window received it — that limit is real, which is why the line carries the `event` name rather than only an outcome. |
| `degraded` | Any place a failure was swallowed and a polite default returned. See below. |
| `dictation.finish` | `outcome` plus `reason` when nothing was produced. |

## `paste.verify` is the important one

`paste.result {"ok":true}` only means the events were handed to the window
server. `CGEventPost` returns `void`: it reports success even when every event
is dropped. `paste.verify` reads the focused text field back and compares it to
a snapshot taken before injection.

```json
{"ax_readable":true,"changed":true,"before_chars":0,"after_chars":87,
 "delta_chars":87,"expected_chars":87,"settled_ms":48}
```

- `changed:true` — the text actually landed. `settled_ms` is how long the
  target took to consume the events; `delta_chars` next to `expected_chars`
  tells you whether all of them arrived.
- `ax_readable:false` — the target's text can't be read (most Electron apps,
  and every non-macOS build). `changed` is meaningless here; it is **not**
  evidence of failure.
- `ax_readable:true` with `changed:false` — **the keystrokes were swallowed.**
  Look for a `paste.modifiers` line immediately before it — it names the
  modifier that was held (`Fn/Globe`, `Command`, …) when we injected.

The check re-reads for up to 600 ms rather than once, because the events sit
in the HID queue and the target consumes them on its own run loop. It runs in
a spawned task, so the `paste.verify` line can appear slightly after
`dictation.finish` — it still carries the dictation's id.

`changed` rather than "grew": typing over a selection replaces it, so a
successful paste can leave the field shorter than it started.

## Aborted dictations

Every path that ends without text writes `dictation.finish` with an
`outcome:"aborted"` and a stable `reason` slug:

| `reason` | Meaning |
|---|---|
| `recording_empty` | Valid WAV, no samples. The audio callback never fired. |
| `dead_capture` | Samples were written, and **every one of them is zero**. See below. |
| `silent_audio` | Below the RMS floor — Whisper was skipped deliberately, to avoid it hallucinating on silence. |
| `whisper_error` | The API failed. `error_category` and `status_code` say how. |
| `no_speech` | Whisper returned nothing, twice. |
| `hallucination` | Whisper returned real characters and the filter dropped all of them. |
| `glossary_ghost` | A short recording that transcribed to nothing but dictionary words. |
| `prompt_introducer_leak` | A short recording starting with "Glossary"/"Glossaire"/… |
| `audio_too_large`, `wav_invalid`, `no_api_key` | Self-explanatory. |

Each of these also writes one WARN line to `ttp.log` naming the trace id, so a
user who only attaches `ttp.log` to a bug report still shows that a dictation
was dropped and why.

## `degraded` — the failures that were swallowed

Seven defects in this app stayed invisible for weeks because every layer
degraded politely: `unwrap_or_default`, `.ok()`, `if let Ok(..)` with no
`else`. The behaviour is often right — a dictation should not die because the
audio backup failed — but until Polaris the fallback was taken in silence.

Every such site on the dictation path now writes one line first. The
behaviour is unchanged; only the silence is.

```sh
grep ' degraded ' ttp-trace.log
```

| `site` | What was swallowed |
|---|---|
| `audio.convert` | Stereo→mono conversion failed; the original WAV was uploaded instead. Six times the bytes, and much closer to the 25 MB ceiling. |
| `audio.size` | The converted file's size could not be read; every size check below it ran against the pre-conversion number. |
| `backup.audio` | The pre-API audio backup failed. A later API failure now loses the recording the user would have retried. |
| `input_mode` | The `AppState` lock was busy — the same contention that makes hotkey presses vanish. |
| `clipboard.restore` | The user's pre-record clipboard was not put back. Their transcription is sitting in it instead. |
| `history.save` | The transcription was pasted but not recorded. |
| `keychain.secret` | The keychain was unavailable and the **shared legacy constant** was used to sign local records. The per-machine tamper resistance is off for this session. |
| `keychain.secret_write` | A freshly generated secret could not be persisted, so the next launch generates another one and every record signed with this one stops verifying. |
| `keychain.migration_flag` | The migration flag could not be read; we assumed "not migrated", which keeps the legacy verification path alive. |
| `keychain.csprng` | The OS CSPRNG failed. Exotic, and worth knowing about. |
| `settings.fsync` | `sync_all` on the temp settings file failed, so the atomic-rename write installed a file whose bytes are not on disk. `architecture.md` advertises "a crash mid-write can never corrupt the live settings file"; this line is that guarantee reporting its own absence. `installed:false` — the temp file was removed and the error was returned as well as recorded. |
| `capture.reclaim` | The Idle backstop could not close an orphaned capture: either `STATE`'s lock was poisoned, or the arbiter and `STATE` disagreed about whether a capture was live. **This one is not benign.** The whole 11-hour-microphone guarantee rests on those two agreeing, so a line here means the guarantee is unverified for that cycle. `check_trace.py` reports it as `capture-arbiter-left-live`. |

A `degraded` line is not an error. It is the sentence "we carried on without
this", written down.

## Seeing the text itself

By default the trace records only a character count and an 8-hex-digit digest
(`sha8`) for each text payload. That is enough to tell "the filter dropped it"
from "the filter rewrote it" — matching digests across two stages mean the text
passed through untouched — without writing anyone's transcriptions to disk.

To see the text, turn on **Settings → Advanced → Diagnostics → Record
transcription text in the trace**, or launch with `TTP_DIAGNOSTICS=1` for a
single session. `dictation.start` records which mode was active via `verbose`.

## Useful greps

```sh
cd ~/Library/Application\ Support/com.ttp.desktop

# Every dictation that produced no text, with the reason
grep '"outcome":"aborted"' ttp-trace.log

# Every dictation whose keystrokes were verifiably swallowed
grep 'paste.verify' ttp-trace.log | grep '"ax_readable":true' | grep '"changed":false'

# The input layer breaking and recovering
grep -E 'hotkey\.(tap_rearmed|stale_fn_cleared|tap_create_failed)' ttp-trace.log

# The app being suspended out from under a dictation
grep 'hotkey.timer_stall' ttp-trace.log

# One dictation, end to end
grep '0007-3f2a' ttp-trace.log

# Slowest stage of each dictation
grep 'dictation.finish' ttp-trace.log
```

## When a dictation takes minutes

`dictation.finish` carries the total `ms`. If that number is wildly larger
than the sum of the stages, read the elapsed column down the dictation and
find the jump.

Everything after `paste.result` is trivial synchronous bookkeeping — a
clipboard write, a JSON append, a few `remove_file` calls. None of it can
take seconds, let alone minutes. So a large gap in that region does not mean
"that step is slow"; it means the process stopped running. Cross-reference
against `hotkey.timer_stall`: if a stall covers the same window, the app was
suspended. This is not hypothetical — a dictation on 27 August showed a
4.2-second hole between `ui.completed` and `usage.recorded` (one JSON write
apart) with `timer_stall {"gap_ms":4178}` covering it, on a completely
separate scheduling context, followed immediately by
`tap_rearmed {"reason":"timeout"}`. Two independent contexts do not stall for
the same 4.2 seconds because one function was slow.

That is why `crate::activity` now holds an `NSProcessInfo` activity
assertion from the moment recording starts until the state machine returns to
Idle. App Nap targets `LSUIElement` agents like TTP, and a napped process
does not slow down — it stops, which is also how the event tap gets disabled
for timeout and the Fn key dies.

`re_arm_tap` writes at most one line per 30 s to `ttp.log` while the tap is
flapping, carrying a `streak` count. The trace keeps every occurrence.

### The keychain, and how to tell it is single-flighted

`keychain.slow` fires above 50 ms and names the `op` and the `account`.
Historically that account read for tens of seconds *between the user's last
word and their text appearing*, and the reason was a check-then-act: the cache
was consulted, the lock released, and only then the unbounded read performed —
so every caller that arrived before the first one finished missed the cache
and made its own securityd call. One process, one lifetime cache, and eight
`secret_read` lines for `usage_hmac_secret`:

    62,304 / 13,612 / 397,600 / 97,407 / 157 / 56,037 / 106 / 86 ms

`keychain::read_once` now performs at most one call however many callers
arrive together, and `lock_wait_ms` is how you check that from the log:

- **one** line with `lock_wait_ms:0`, and any others carrying a non-zero
  `lock_wait_ms`, is coalescing working — those callers waited for someone
  else's read rather than paying again;
- **several** lines with `lock_wait_ms:0` for the same account in one session
  is the old shape back. `check_trace.py` reports it as
  `keychain-not-single-flighted`.

A slow read is still worth reading even when it coalesced: single-flighting
removes the multiplier, not the securityd cost. That is what `warm_caches`
and `keychain.warmed` are for — paying it at launch, on a thread nobody is
waiting on.

One caveat when counting: sessions in the log are the spans between
`app.launched` lines, and a dev or test binary appending to the same
`ttp-trace.log` contributes reads to whichever span it lands in without a
launch line of its own. Check the timestamps against what was running before
attributing a burst to the app.

## When the microphone was left on

On 2026-08-30 a 60 ms tap left a capture live for **ten hours and fifty-seven
minutes**. The trace of it is five lines long and reads perfectly ordinary:

```
23:18:21.386  hotkey.press                          → Recording
23:18:21.446  hotkey.release                        → Processing
23:18:21.852  capture.stop_failed "No recording in progress"
23:18:21.854  state.transition Processing → Idle
23:18:21.930  capture.start   {"device":"AirPods Pro"}
```

The stop ran, found nothing, and went home 78 ms before the start published a
stream that only the stop could have closed. Nothing said the microphone was
on, so nothing was noticed until `hotkey.timer_stall {"gap_ms":963122}` the
next morning.

`src-tauri/src/capture_arbiter.rs` now enforces the property that was being
assumed: **a live capture exists only while the state machine wants one.** It
is driven from `AppState::set_state` and uses no clock anywhere, so it cannot
be defeated by a slow Bluetooth device or a future polled late.

What that means for reading the log:

- The **old** failure is a `capture.start` with no `capture.stop` after it.
  That is what the five lines above are.
- The **fix engaging** is `capture.orphan_prevented` or
  `capture.orphan_reclaimed`. Both are written *after* the cpal stream has
  been dropped, so either line is the microphone reporting that it went off.
- A refused start still writes its `capture.start` first — the refusal
  happens after the stream is built. So `capture.start` followed by
  `capture.orphan_prevented` and no `capture.stop` is the healthy shape, not
  a leak. Anything reading only for `capture.stop` will get this exactly
  backwards and report the fix as the bug.
- The shapes that would mean the guarantee has failed: a second close with no
  `capture.start` between (the stream the arbiter said it dropped was still
  there), `degraded {"site":"capture.reclaim"}` (the arbiter and `STATE`
  disagree), and `capture.stop_waited_for_start {"timed_out":true}` (the
  settle window elapsed, so the arbiter is now the only guard). All three are
  checked as `capture-arbiter-left-live` — see `docs/trace-invariants.md`.

```sh
# The fix engaging, and what it refused
grep -E 'capture\.(orphan_prevented|orphan_reclaimed|stale_dropped)' ttp-trace.log

# The guarantee failing
grep 'capture.reclaim' ttp-trace.log
grep 'capture.stop_waited_for_start' ttp-trace.log | grep '"timed_out":true'
```

## Telling a dead microphone from a quiet room

`silent_audio` and `dead_capture` both end a dictation with no text, and used
to be the same code path with the same message — "no speech detected". That
message is wrong and actively misleading in one of the two cases: it tells
someone who just dictated for eight seconds that they had not spoken, when
what actually happened is that their microphone handed us nothing.

A live microphone always has a noise floor. In a silent room the RMS lands
somewhere around 0.0005–0.003 and individual samples are never all zero. So:

- `nonzero_ratio > 0` with `avg_rms` below `floor` → **a quiet room**. Reason
  `silent_audio`, message "no speech detected". Correct.
- `nonzero_ratio == 0` → **the device delivered digital silence**. Reason
  `dead_capture`, message about an empty recording (check permissions, check
  whether another app has the mic).

The predicate is deliberately strict: a single non-zero sample anywhere in
the recording disqualifies `dead_capture`. Telling a user their microphone is
broken is a strong claim and must not fire on a merely very quiet take.

### Which device was it

Both reasons carry `device`, and `capture.start` / `capture.stop` bracket the
recording:

```sh
# Every dead capture, with the microphone responsible
grep '"reason":"dead_capture"' ttp-trace.log

# Did the default input move while the user was talking?
grep 'capture.stop' ttp-trace.log | grep '"device_changed":true'
```

This is what separates the two failure modes that look identical from the
outside:

- `dead_capture` on a **Bluetooth device** (AirPods and friends) — the
  headset was connected but never actually streaming. Intermittent by nature:
  the same headset works on the next attempt. Look for `capture.stop` with
  `samples` well above zero — the stream was running and delivering buffers,
  they were just full of silence.
- `dead_capture` on the **built-in microphone** — permission revoked, or
  another process holding the device exclusively. Usually `samples` is zero
  too, because the callback never fired at all.
- `device_changed: true` on `capture.stop` — the OS default moved mid
  recording. An already-open cpal stream does not follow it, so it keeps
  reading from a device that has stopped producing audio.

## Retention

~10 MB across four files at 2.5 MB each. The number that matters is not the
total but the **three rotated files**: the live one can be nearly empty right
after a rotation, so the guaranteed floor is 7.5 MB.

Measured over the 477-dictation corpus of 26 August – 2 September 2026, the
observed cost is **4.9 KB per dictation** (2.9 KB of it in the dictation's own
staged lines, the rest its share of the standalone stream) at a rate of **~73
dictations a day** over the elapsed window, peaking at 131 on 30 August. That
floor therefore holds about **1,530 dictations, or roughly three weeks** —
the projection stands.

**It stands for the binary that produced the corpus, and that binary is not
the current one.** No line in those 477 dictations carries `dur_ms`, and none
of `settings.snapshot`, `filter.*`, `keychain.api_key`, `clipboard.write`,
`vad.*` or `hotkey.tap_health` appears anywhere in it. Every one of those is
in the code and none has ever run on the machine being harvested, so 4.9 KB is
a measurement of the *previous* verbosity, and the 5.1 KB this section used to
claim was an estimate that was never observed either.

Adding up what the current code writes per dictation and is missing from that
measurement — `dur_ms` on every staged line (~11 B × ~24), `settings.snapshot`
(~280 B), the three `filter.*` verdicts (~450 B), `keychain.api_key`,
`clipboard.write` and `usage.polish_recorded` (~340 B), and the five-minute
`hotkey.tap_health` heartbeat amortised across a day's dictations (~360 B) —
puts it near **6.6 KB**, and nearer 7 KB with VAD armed. At that cost the
floor holds ~1,140 dictations: **about 15–16 days at 73 a day, and 8 or 9 at
the observed peak rate.**

So: the three-week window is **projected to fail** the moment the current
build is installed, and to fail hardest in exactly the weeks a heavy user
generates the most evidence. It has not failed yet, and the number above is
arithmetic on stage sizes rather than a measurement — the honest form of the
claim is that the next harvest should re-measure this from the log rather
than trust either figure. If it needs fixing, the cheap lever is
`KEEP_TRACE_ROTATIONS` (three files today), not deleting stages.

The standalone stream costs on top of that: the event-tap heartbeat is the
only periodic writer, at five-minute intervals and ~90 bytes, so ~26 KB a day.

If you add a stage that fires per dictation, add ~150 bytes — the observed
average line length in the real log, not the ~100 this section used to
assume — and redo this arithmetic. Losing history to verbosity would defeat
the point.

## What is not covered

Honest limits, so nobody reads silence as proof of health. Several items that
used to be on this list have moved off it — `vad.*`, `settings.snapshot`,
`hotkey.tap_health` and the `degraded` family exist now. What remains:

- **The frontend.** The JS side drives `stop_recording` → `process_audio`. An
  exception in that handoff leaves `capture.stop` with no `dictation.start`
  after it. That gap is visible, but the reason for it is not. This is still
  the one shape the trace can only bound, not explain, and it is the largest
  remaining hole: nothing in `src/` writes to the trace.
- **When a setting changed.** `settings.snapshot` now records the
  configuration each dictation ran under, so two dictations can be compared —
  but the trace still does not record the moment a user flipped a switch. The
  `companion.*` events are the exception, and only for the fields they cover.
- **Anything before `app.launched`.** A crash during Tauri setup leaves
  nothing.
- **Anything after the last line.** The writer is a background thread with a
  bounded queue. A hard kill (`panic = "abort"`, SIGKILL) can lose whatever was
  queued and not yet written — at most a few lines, and precisely the last few,
  which is the worst place to lose them. A queue overflow is reported as
  `trace_dropped_lines` on the next line through, and a rejected write as
  `trace_write_failures`; a process death is not reported at all, because
  there is nobody left to report it.
- **The audio callback.** `capture.start` and `capture.stop` bracket the
  recording and `audio.signal` measures the result, but the cpal callback
  itself is untraced by design — it is a real-time audio thread and a channel
  push is not free enough to put in it. A stream that delivers buffers late
  rather than not at all is still invisible.
- **Whether the paste landed, on Windows and in Electron apps.**
  `paste.verify` needs Accessibility to read the target back; where it cannot,
  `ax_readable:false` is an honest "unknown", not evidence.
- **Cause, everywhere.** The trace records what the app decided and how long it
  took. It does not record why macOS disabled the tap, why the keychain took
  seven seconds, or why the Bluetooth headset sent silence. It narrows those
  to one component and one moment, which is the whole job, and then stops.
