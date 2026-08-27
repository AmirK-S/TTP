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
2 MB, keeping `ttp-trace.log.1` … `.3`.

Unlike `ttp.log`, the trace is **not** filtered by log level. Release builds
default to `warn`, which is precisely why every silent-drop path used to be
invisible; gating the trace behind that threshold would reproduce the blind
spot it was written to remove.

## Line format

```
[2026-08-26 08:48:57.412] [0007-3f2a] +    0ms dictation.start  {"kind":"recording","verbose":false}
[2026-08-26 08:48:57.418] [0007-3f2a] +    6ms audio.duration   {"secs":7.52,"wav_bytes":481324}
[2026-08-26 08:48:58.902] [0007-3f2a] + 1490ms whisper.response {"chars":87,"sha8":"9f2c1ab0","attempt":1,"ms":1484}
[2026-08-26 08:48:59.118] [0007-3f2a] + 1706ms paste.verify     {"ax_readable":true,"changed":true,"delta_chars":87,"expected_chars":87,"settled_ms":48}
[2026-08-26 08:48:59.121] [0007-3f2a] + 1709ms dictation.finish {"outcome":"pasted","ms":1709,"chars":87,"words":16}
```

- `[0007-3f2a]` — the trace id. `grep 0007-3f2a ttp-trace.log` gives you that
  one dictation and nothing else. The `0007` is a per-session sequence number,
  so ids sort in the order the dictations happened.
- `+1490ms` — elapsed since the dictation began, so slow stages are obvious.
- `[········]` in the id column marks a standalone event (a hotkey press, an
  event-tap recovery) rather than a dictation stage.

## The stages

| Stage | What it tells you |
|---|---|
| `app.launched` | Session boundary, with version and platform. Everything below it belongs to one run of the app. |
| `hotkey.press` / `hotkey.release` | The Fn/Globe key was seen. **No line here means the input layer never fired** — the recording never started. |
| `hotkey.event_dropped` | A hotkey event arrived while the state lock was held and was discarded. The press happened; nothing came of it. |
| `state.transition` | Every move between Idle / Recording / Processing. A session parked in Processing makes all later presses silent no-ops. |
| `capture.start_failed` / `capture.stop_failed` | Recording never started, or the finished recording could not be retrieved. Covers all nine early exits in the capture layer. |
| `dictation.rejected` | Audio was captured and then thrown away before transcription — rate limit, or a path that failed validation. |
| `hotkey.tap_rearmed` | macOS had disabled our event tap and we re-armed it. Every Fn press between the disable and this line was lost. |
| `hotkey.stale_fn_cleared` | The Globe key was latched "held" and we forced it down. Keystrokes injected before this were being routed to the Globe shortcut layer. |
| `hotkey.timer_stall` | The 20 ms poll timer skipped `gap_ms`. The process was descheduled — nothing advanced during that window: no hotkey, no state machine, no in-flight dictation. TTP now holds an activity assertion for the whole Recording → Idle window (see `crate::activity`), so a stall spanning a dictation should no longer be possible; one that still appears is worth investigating. |
| `capture.start` | Which microphone actually served the recording, its rate/channels/format, whether it is the OS default, and what the user had asked for. |
| `capture.stop` | Samples the callback delivered, and whether the OS default input changed while the user was talking. |
| `audio.duration` / `audio.signal` | How much audio, how loud. `avg_rms` below `floor` means the silence gate will drop it; `peak` and `nonzero_ratio` distinguish a quiet room from a dead device. |
| `audio.convert` | Stereo 48 kHz → mono 16 kHz, and the size change. |
| `whisper.request` / `whisper.response` | Bytes sent, language pinned, latency, and how many characters came back. `attempt:2` means the first call returned an empty body. |
| `cleanup`, `polish`, `dictionary` | Each text transformation, with `changed` and before/after character counts. A `to.chars` of 0 names the stage that emptied the transcription. `polish.outcome` is `applied` / `failed` / `guard_rejected` / `skipped` — what actually happened, not whether it was allowed to try. |
| `polish.outage` | Polish has failed `consecutive_failures` times in a row against `model`. Emitted on every failure; the user is notified once per session at three. |
| `paste.accessibility` | `tcc_trusted` vs `ax_probe_ok`. Trusted-but-not-working is the stale-TCC state left behind by in-place app updates. |
| `paste.decision` | `type` (direct keystrokes) or `clipboard` (Cmd+V), and how many characters. |
| `paste.modifiers` | A modifier key was still held at injection time. Only emitted when one was. |
| `paste.result` | Whether the events were posted. |
| `paste.verify` | Whether they **landed**. See below. |
| `clipboard.restore` | The user's pre-record clipboard was put back. |
| `correction_window.started` | The dictionary correction watcher was armed. |
| `history.saved`, `usage.recorded`, `files.cleaned` | Post-paste bookkeeping. All trivial and synchronous — a large jump between any two of these means the process stalled, not that the step is slow. |
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

## What is not covered

Honest limits, so nobody reads silence as proof of health:

- **The frontend.** The JS side drives `stop_recording` → `process_audio`. An
  exception in that handoff leaves `capture.stop` with no `dictation.start`
  after it. That gap is visible, but the reason for it is not.
- **VAD auto-stop.** Off by default; when on, the decision to cut a recording
  short is not recorded.
- **Settings changes.** A dictation's behaviour depends on settings read at
  the time; the trace shows the resulting values (`whisper.request.lang`,
  `polish.decision`) but not when the user changed them.
- **Anything before `app.launched`.** A crash during Tauri setup leaves
  nothing.
- **Rotation.** ~8 MB across four files, roughly three weeks of heavy use.
  Older evidence is gone, which matters for "it happened last month".

A dictation that leaves `capture.stop` with no `dictation.start` after it is
the one shape the trace can currently only bound, not explain.
