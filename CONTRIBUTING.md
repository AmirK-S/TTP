# Contributing to TTP

Thanks for taking the time to dig into TTP. This doc covers what you need
to know before sending a PR or opening an issue.

> **License note:** TTP is source-available, not open-source. See
> [LICENSE](LICENSE) for what you can and can't do with the source. In
> short: read it, build it for yourself, send PRs upstream — yes; ship a
> competing app or strip the licensing layer — no.

## Quick start

```bash
git clone https://github.com/AmirK-S/TTP.git
cd TTP
npm install
npm run tauri dev   # starts the dev shell
```

Prerequisites: Node 18+, Rust stable (the CI pins 1.85.0, anything ≥ that
will work locally), Tauri CLI (`cargo install tauri-cli` or rely on the
`@tauri-apps/cli` npm devDep).

## Before you open a PR

Three checks must pass:

1. `npm run i18n:check` — locale parity + every `t('xxx')` callsite
   resolves against a real key in `en.json`. Adding a new UI string?
   Add it to BOTH `src/i18n/locales/en.json` AND `src/i18n/locales/fr.json`.
2. `npm run test:run` — vitest suite (frontend pure helpers + hook
   integration tests). Adding a new pure helper? Add tests next to it
   (`my-helper.ts` + `my-helper.test.ts`).
3. `cd src-tauri && cargo test --lib` — Rust unit tests. Adding a new
   decision rule in Rust? Extract it as a pure function and write tests
   alongside it (see `fnkey_fsm.rs` for the pattern).

The CI runs all three on every PR.

## Conventions

### Translation contract

User-facing strings cross the Rust ↔ JS boundary as **keys**, not as
literal English text. Rust modules return strings like
`"error.microphone_permission_denied"` and the React side resolves them
via i18next. See `docs/architecture.md` § "i18n contract" for the rules.
The parity scripts catch most violations at build time.

### State changes

Recording lifecycle state lives in `AppState` (`src-tauri/src/state.rs`).
The mutex is held only briefly inside `set_state`; consumers must NOT
re-enter it from inside side-effect helpers (`tray::hide_pill`,
`audio_monitor::start`, etc.). If you need to take an action on a state
transition, add it inside `set_state` or schedule it via an emitted event.

Persistent vs transient state is split deliberately. `hands_free_mode` is
the persisted preference; `session_hands_free` is the per-recording
override. Never mutate the persistent field to express a transient state
— it leaks back into the user's settings UI and tray menu.

### Tests

Pure functions are the unit of testing. When you find a decision point
buried inside an objc closure / async invocation / mutex guard, extract
it to a pure helper and test the helper. The existing examples:

- `fnkey_fsm::fn_decide` — the entire Fn-key state machine, exercised
  without touching the macOS event loop.
- `fnkey_fsm::FnFsmState` — the snapshot type the wrapper marshals atomics
  into and out of.
- `pipeline::classify_transcription_error` and `classify_polish_error`
  — pure HTTP error → analytics category mapping.
- `licensing::is_pro_at(record, now)` — pure Pro-entitlement decision
  taking the clock as a parameter.
- `vad::vad_step` and `vad::required_ticks` — VAD energy gate math.
- `src/lib/updater-decisions.ts` — every gate in `useUpdater` extracted.
- `src/lib/audio-stream-error.ts` — capture vs monitor disambiguation.

### Style

Prefer `crate::logging::log_info` / `log_warn` / `log_error` over
`eprintln!` and `println!`. The logger respects `$TTP_LOG_LEVEL`, scrubs
PII, and writes to the file the user shares when triaging a bug.

No em-dashes in user-facing copy. Project copy voice rule, enforced by
review. (Comments and internal logs are fine.)

No `// TODO` placeholders left in shipped code — if you can't finish,
open an issue and reference it.

## Areas that need help

If you're looking for a starting point:

- **Frontend tests** — the integration coverage for `useRecordingControl`
  and `useUpdater` is still partial. Driving the hook through realistic
  Tauri-event sequences via `renderHook` is high-value.
- **`process_recording` orchestration tests** — extract a `PipelineDeps`
  trait so the master state machine in `transcription/pipeline.rs` can be
  tested against a mock Whisper + mock paste. The classification helpers
  are already tested; the orchestration isn't.
- **F3 / system-key edge cases on third-party keyboards** — `fnkey_fsm`
  has the timing rules under test but real keyboards vary. Bug reports
  with reproduction steps are extremely useful here.
- **macOS audio device picker** — currently TTP always uses the default
  input device. Multi-mic setups would benefit from a picker.

## Reporting bugs

Open a GitHub issue with:

1. OS + TTP version (Settings → About has the build SHA).
2. Steps to reproduce.
3. The relevant chunk of `ttp.log` (Settings → Diagnostics → Show log
   folder). The log redacts API keys, file paths, emails, and tokens —
   you can paste it straight in.

Security issues: see [SECURITY.md](SECURITY.md) — email instead of opening
a public issue.
