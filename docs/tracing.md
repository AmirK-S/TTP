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
[2026-08-26 08:48:59.118] [0007-3f2a] + 1706ms paste.verify     {"ax_readable":true,"before_chars":0,"after_chars":87,"grew":true,"expected_chars":87}
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
| `hotkey.press` / `hotkey.release` | The Fn/Globe key was seen. **No line here means the input layer never fired** — the recording never started. |
| `hotkey.tap_rearmed` | macOS had disabled our event tap and we re-armed it. Every Fn press between the disable and this line was lost. |
| `hotkey.stale_fn_cleared` | The Globe key was latched "held" and we forced it down. Keystrokes injected before this were being routed to the Globe shortcut layer. |
| `audio.duration` / `audio.rms` | How much audio, how loud. `avg_rms` below `floor` means the silence gate will drop it. |
| `audio.convert` | Stereo 48 kHz → mono 16 kHz, and the size change. |
| `whisper.request` / `whisper.response` | Bytes sent, language pinned, latency, and how many characters came back. `attempt:2` means the first call returned an empty body. |
| `cleanup`, `polish`, `dictionary` | Each text transformation, with `changed` and before/after character counts. A `to.chars` of 0 names the stage that emptied the transcription. |
| `paste.accessibility` | `tcc_trusted` vs `ax_probe_ok`. Trusted-but-not-working is the stale-TCC state left behind by in-place app updates. |
| `paste.decision` | `type` (direct keystrokes) or `clipboard` (Cmd+V), and how many characters. |
| `paste.modifiers` | A modifier key was still held at injection time. Only emitted when one was. |
| `paste.result` | Whether the events were posted. |
| `paste.verify` | Whether they **landed**. See below. |
| `dictation.finish` | `outcome` plus `reason` when nothing was produced. |

## `paste.verify` is the important one

`paste.result {"ok":true}` only means the events were handed to the window
server. `CGEventPost` returns `void`: it reports success even when every event
is dropped. `paste.verify` reads the focused text field back and compares it to
a snapshot taken before injection.

```json
{"ax_readable":true,"before_chars":0,"after_chars":87,"grew":true,"expected_chars":87}
```

- `grew:true` — the text actually landed.
- `ax_readable:false` — the target's text can't be read (most Electron apps).
  `grew` is meaningless here; it is not evidence of failure.
- `ax_readable:true` with `grew:false` — **the keystrokes were swallowed.**
  Look for a `paste.modifiers` line immediately before it — it names the
  modifier that was held (`Fn/Globe`, `Command`, …) when we injected.

## Aborted dictations

Every path that ends without text writes `dictation.finish` with an
`outcome:"aborted"` and a stable `reason` slug:

| `reason` | Meaning |
|---|---|
| `recording_empty` | Valid WAV, no samples. Usually a silently revoked mic permission, or the mic held exclusive by another app. |
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
grep 'paste.verify' ttp-trace.log | grep '"ax_readable":true' | grep '"grew":false'

# The input layer breaking and recovering
grep -E 'hotkey\.(tap_rearmed|stale_fn_cleared|tap_create_failed)' ttp-trace.log

# One dictation, end to end
grep '0007-3f2a' ttp-trace.log

# Slowest stage of each dictation
grep 'dictation.finish' ttp-trace.log
```
