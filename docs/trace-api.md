# `ttp-trace.log` — the read API

The contract between the Rust backend (`src-tauri/src/trace_api.rs`) and any
frontend that displays the dictation trace. Five commands and one event
channel. If anything here disagrees with the code, the code is wrong — change
this file first.

Every type below is what `serde_json` produces from the Rust structs, which is
what arrives at `invoke()`. Field names are exactly as written: **snake_case,
no camelCase conversion anywhere.**

---

## 0. The one thing to understand first

There is no database. The commands parse `ttp-trace.log` and its three rotated
siblings on every call. That has three consequences the UI has to respect:

1. **A query costs a file read.** ~10 MB worst case, a few hundred
   milliseconds. Fine for a mount, a refresh button, or a search. Not fine in
   a `useEffect` that reruns on every keystroke, and not fine on a timer —
   use the live channel for that instead.
2. **History survives restarts,** and reaches back roughly three weeks (see
   `trace_status` for the real numbers on this machine).
3. **What you see is what is in the file the user can attach to a bug
   report.** The viewer is a reader, not a second source of truth.

---

## 1. Types

### `TraceEvent`

One line of the log.

```json
{
  "ts": "2026-08-31 09:12:03.101",
  "id": "0007-3f2a",
  "elapsed_ms": 1490,
  "stage": "whisper.response",
  "fields": { "chars": 87, "sha8": "9f2c1ab0", "attempt": 1, "ms": 1484, "dur_ms": 1484 }
}
```

| Field | Type | Notes |
|---|---|---|
| `ts` | `string` | Local wall clock, `YYYY-MM-DD HH:MM:SS.mmm`. Not ISO-8601 and not UTC — it is the same string the log file shows, deliberately, so a user reading both sees the same timestamps. Parse with care if you need a `Date`. |
| `id` | `string \| null` | The dictation id. `null` for standalone events (`hotkey.*`, `state.transition`, `app.launched`, `companion.*`, `vad.*`, `capture.*`, `keychain.*`, `permission.*`, `degraded`). |
| `elapsed_ms` | `number \| null` | Milliseconds since that dictation began. `null` whenever `id` is `null`. |
| `stage` | `string` | Dotted stage name. See `docs/tracing.md` for the vocabulary. |
| `fields` | `object` | Always an object, never `null`. Contents vary per stage. |

**`fields` is untyped on purpose.** Stages gain fields as defects are found, and
a UI that hard-codes a schema per stage breaks every time the backend learns
something. Render it generically — a key/value table — and special-case only
the handful of stages worth a custom row (`dictation.finish`, `paste.verify`,
`audio.signal`).

Two keys appear on nearly every line and are worth first-class treatment:

- **`dur_ms`** — milliseconds since the *previous* stage of the same
  dictation. Present on every stage of a dictation. This is the "where did the
  time go" number; a bar proportional to it next to each row is the single most
  useful thing a viewer can draw.
- **`ms`** — present on stages that timed a specific call
  (`whisper.response`, `keychain.api_key`, `paste.result`, `polish`, …). Where
  both exist and disagree, `dur_ms` includes the gap before the call started
  and `ms` does not.

### `DictationTrace`

One dictation, assembled from its lines.

```json
{
  "id": "0007-3f2a",
  "started_at": "2026-08-31 09:12:01.392",
  "outcome": "pasted",
  "reason": null,
  "total_ms": 1709,
  "chars": 87,
  "words": 16,
  "slowest_stage": "whisper.response",
  "slowest_stage_ms": 1484,
  "stages": [ /* TraceEvent, in the order written */ ]
}
```

| Field | Type | Notes |
|---|---|---|
| `id` | `string` | |
| `started_at` | `string` | `ts` of the first line carrying this id. |
| `outcome` | `string \| null` | `"pasted"`, `"clipboard_fallback"`, `"aborted"`. **`null` means there is no `dictation.finish` line** — either still in flight, or the process died mid-dictation. Render that as its own state; it is a finding, not a loading spinner. |
| `reason` | `string \| null` | The abort slug when `outcome == "aborted"`: `silent_audio`, `dead_capture`, `hallucination`, `no_speech`, `glossary_ghost`, `prompt_introducer_leak`, `whisper_error`, `recording_empty`, `wav_invalid`, `audio_too_large`, `no_api_key`, `api_key_read_failed`, `clipboard_write_failed`, `audio_file_missing`. |
| `total_ms` | `number \| null` | From `dictation.finish`. |
| `chars`, `words` | `number \| null` | What was finally inserted. Absent on aborts. |
| `slowest_stage` | `string \| null` | Stage with the largest `dur_ms`, excluding `dictation.start`. Computed in Rust so every consumer agrees. |
| `slowest_stage_ms` | `number \| null` | Its `dur_ms`. |
| `stages` | `TraceEvent[]` | Chronological. Includes `paste.verify`, which is written from a spawned task and can therefore appear **after** `dictation.finish` — do not assume the last element is the terminal line. |

### `TraceStatus`

```json
{
  "verbose": false,
  "live": true,
  "channel": "trace-event",
  "files": [
    { "name": "ttp-trace.log",   "bytes": 812443 },
    { "name": "ttp-trace.log.1", "bytes": 2500118 }
  ],
  "total_bytes": 3312561,
  "rotate_at_bytes": 2500000,
  "keep_rotations": 3
}
```

`verbose` is the privacy-critical one: `true` means full transcription text is
being written to disk (`diagnostics_enabled`, or `TTP_DIAGNOSTICS=1`). **Say so
prominently in the UI.** A user who does not know their speech is on disk
cannot consent to it.

---

## 2. Commands

### `trace_recent_dictations`

```ts
invoke<DictationTrace[]>('trace_recent_dictations', { limit?: number })
```

Newest first. `limit` defaults to 50, is clamped to `[1, 500]`, and counts
*dictations*, not lines. Each result carries its complete `stages` array — one
call is enough to render both a list and any detail view opened from it.

Returns `[]` when the log is empty or unreadable. It never throws; a trace
viewer that can fail to load is a viewer you cannot use to debug the failure.

### `trace_get_dictation`

```ts
invoke<DictationTrace | null>('trace_get_dictation', { id: string })
```

Scans the entire retained window — use it for "paste an id from a bug report",
not for rendering a list. `null` when the id is not in the window (it may have
rotated out; `trace_status` says how far back the window reaches).

### `trace_recent_events`

```ts
invoke<TraceEvent[]>('trace_recent_events', {
  limit?: number,          // default 50, clamped to [1, 500]
  stagePrefix?: string,    // e.g. "hotkey.", "companion.", "degraded"
})
```

The raw stream, newest first, including standalone events that belong to no
dictation. This is how you render the input timeline beside the dictation list.

**Argument naming:** Tauri converts JS `stagePrefix` to Rust `stage_prefix`
automatically. Pass `stagePrefix` from JS.

Useful prefixes:

| Prefix | Answers |
|---|---|
| `hotkey.` | Did the input layer see the key? Is the event tap alive? |
| `capture.` | Which microphone, and did it deliver samples? Also whether the arbiter had to close one (`capture.orphan_prevented` / `capture.orphan_reclaimed` / `capture.stale_dropped`). |
| `state.transition` | Is the state machine parked in `Processing`? |
| `degraded` | Every place a failure was swallowed and replaced with a default. |
| `keychain.` | The latency class that once wedged the app for 7.6 s. |
| `companion.` | The face survival test — see `docs/companion-faces-design.md` §2. |
| `vad.` | Was the recording cut short by auto-stop rather than by the user? |
| `permission.` | Was the user's Accessibility grant destroyed by a `tccutil reset`, and was the UI ever told a permission was missing? |

### `trace_set_live`

```ts
invoke<void>('trace_set_live', { enabled: boolean })
```

Turns the live channel on and off. **Off by default.** Call `true` on mount and
`false` on unmount — while it is on, the writer thread does an IPC emit per
line, and a viewer nobody is looking at should not cost that.

### `trace_status`

```ts
invoke<TraceStatus>('trace_status')
```

Cheap (four `stat` calls). Safe to poll if you want a live byte counter.

---

## 3. The live channel

```ts
import { listen } from '@tauri-apps/api/event';

await invoke('trace_set_live', { enabled: true });
const un = await listen<TraceEvent>('trace-event', (e) => {
  append(e.payload);          // one TraceEvent per emission
});
// on unmount
un();
await invoke('trace_set_live', { enabled: false });
```

Channel name: **`trace-event`**. Payload: exactly one `TraceEvent`, the same
shape the commands return.

Contract details that matter:

- **Emitted from the writer thread, after the line is on disk.** The channel
  cannot delay a dictation, and anything you receive is already in the file.
- **Ordered.** Single writer thread, single consumer loop.
- **Not replayed.** Subscribing gives you lines from that moment on. Prime the
  view with `trace_recent_dictations` first, then attach the listener; a line
  that arrives between the two calls appears twice, so dedupe on
  `id + stage + ts`.
- **Lossy under extreme pressure, and it says so.** The queue is bounded at
  4096. If it overflows, the dropped count is attached to the next line that
  gets through as `fields.trace_dropped_lines`. Surface that — a gap that
  announces itself is recoverable, a silent one is the bug this whole
  subsystem exists to prevent.
- **`app.launched` is a session boundary.** Draw a divider on it. "Did the app
  restart between these two dictations?" is the question you ask when
  something recovered on its own.

---

## 4. What the UI should not do

- **Do not write to the trace.** There is no command for it and there will not
  be. The log is evidence.
- **Do not display `fields.text`, `fields.from.text` or `fields.to.text`
  without the `verbose` banner from `trace_status`.** Those keys exist only
  when the user opted into full-text capture, and the UI is the last place
  that can make that visible to them.
- **Do not derive a stage vocabulary and reject unknown stages.** New stages
  land whenever a defect is found. Unknown stage names must render, not
  disappear.
- **Do not translate stage names.** They are diagnostic identifiers, not prose.
  They stay in English in both locales, deliberately: they are what a user
  greps for and what a bug report quotes. Any surrounding *prose* — headings,
  empty states, the verbose warning — goes through `src/i18n/locales/` like
  everything else.

---

## 5. Minimal viewer, end to end

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type TraceEvent = {
  ts: string;
  id: string | null;
  elapsed_ms: number | null;
  stage: string;
  fields: Record<string, unknown>;
};

type DictationTrace = {
  id: string;
  started_at: string;
  outcome: string | null;
  reason: string | null;
  total_ms: number | null;
  chars: number | null;
  words: number | null;
  slowest_stage: string | null;
  slowest_stage_ms: number | null;
  stages: TraceEvent[];
};

const [status, recent] = await Promise.all([
  invoke<TraceStatus>('trace_status'),
  invoke<DictationTrace[]>('trace_recent_dictations', { limit: 50 }),
]);

await invoke('trace_set_live', { enabled: true });
const un = await listen<TraceEvent>('trace-event', ({ payload }) => {
  // payload.id === null  → standalone event, goes on the timeline
  // payload.id !== null  → append to that dictation, creating it if new
});
```
