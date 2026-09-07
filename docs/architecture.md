# TTP Architecture

This document describes the runtime architecture of TTP (Talk To Paste). It
is the entry point for new contributors and for triaging unfamiliar areas.
For security boundaries see `THREAT_MODEL.md`. For release procedure see
`SECURITY.md` (key recovery section).

## High-level layout

TTP is a Tauri 2 desktop app: Rust backend (`src-tauri/`) talks to a React
frontend (`src/`) via Tauri's IPC bridge. The Rust side owns every OS
boundary (microphone, keyboard, clipboard, accessibility, networking,
filesystem). The React side renders three independent windows (Settings,
Onboarding, FloatingBar pill) plus an invisible host that handles the
ApiKeySetup modal.

```
            ┌──────────────────────────────────────────────────┐
            │            React (windows + hooks)               │
            │  Settings · Onboarding · FloatingBar · ApiKey    │
            │                                                  │
            │  hooks/useRecordingControl ◀── recording-state-* │
            │  hooks/useTranscription    ◀── transcription-*   │
            │  hooks/useUpdater          ◀── update-progress   │
            │  stores/settings-store     ◀── settings-changed  │
            └──────────────┬───────────────────────────────────┘
                           │  invoke / listen (Tauri IPC)
            ┌──────────────▼───────────────────────────────────┐
            │            Rust backend (src-tauri/)             │
            │                                                  │
            │  shortcuts ── fnkey ── fnkey_fsm (pure FSM)      │
            │       │           │                              │
            │       └─►  state.rs (AppState mutex)             │
            │                   │                              │
            │   audio_capture ──┴── audio_monitor              │
            │       │  (RMS bucket: AtomicU32)                 │
            │       └─►  vad (energy-based auto-stop)          │
            │                                                  │
            │   transcription/  (pipeline → whisper → polish)  │
            │       │                                          │
            │       └─►  dictionary/  history/  usage/         │
            │                                                  │
            │   licensing/  settings/  keychain  credentials   │
            │   logging  telemetry/sentry  i18n                │
            └──────────────────────────────────────────────────┘
```

## Recording lifecycle (the hot path)

1. User presses Fn (macOS) or `Ctrl+Space` (Windows / fallback).
2. macOS only: `fnkey` CGEventTap callback flips `FN_KEY_PHYSICALLY_DOWN`.
3. The 20 ms NSTimer in `fnkey` snapshots the atomics into an
   `fnkey_fsm::FnFsmState`, calls `fn_decide`, writes the result back, and
   dispatches the returned `FnAction`. Debounce (150 ms), double-tap
   (300 ms gap), and hands-free single-tap-stop (400 ms grace) all live
   in the pure FSM and have unit tests.
4. `FnAction::StartRecording` calls `shortcuts::handle_shortcut_event_public(Pressed)`
   which transitions `AppState` to `Recording`.
5. `state.rs::set_state(Recording, app)` does three things:
   * starts `audio_capture::start_recording` (opens cpal stream, writes WAV)
   * starts `audio_monitor` (polls `audio_capture::current_rms()` at 30 fps,
     emits `audio-level` events to the pill window)
   * starts `vad` (polls `current_rms()` at 10 Hz; if VAD opt-in is on,
     triggers `handle_shortcut_event_public(Released)` after N seconds of
     silence — same code path as a real release)
6. On release / VAD fire: `state.rs::set_state(Processing, app)` stops the
   audio threads and `state.rs::set_state(Idle, app)` clears the session
   override fields. The pipeline (kicked off by the JS recording hook)
   then transcribes, polishes, applies the dictionary, and pastes.

## Shared atomics

`audio_capture` and `audio_monitor` used to own separate cpal input streams
in parallel — a recurring source of "silently empty recording" bugs on
CoreAudio. v3.1 consolidates them: `audio_capture` is the only owner of a
cpal stream, computes RMS inside its existing WAV-writing callback, and
publishes via `audio_capture::current_rms()` (backed by `AtomicU32` storing
the bit-pattern of an `f32`). `audio_monitor` and `vad` both poll this
bucket. Reset to 0 happens on every `start_recording` (no leak) and on
`stop_recording` (no stale frame).

## State management contract

`AppState` (`src-tauri/src/state.rs`) carries five pieces of state:

| Field                     | Type             | Meaning                                                       |
| ------------------------- | ---------------- | ------------------------------------------------------------- |
| `recording_state`         | enum             | `Idle` / `Recording` / `Processing`                           |
| `hands_free_mode`         | `bool`           | PERSISTED preference, mirrored from `settings.hands_free_mode`|
| `session_hands_free`      | `Option<bool>`   | TRANSIENT override for the current session                    |
| `last_shortcut_time`      | `Option<Instant>`| Used by `shortcuts.rs` double-tap detection                   |
| `recording_started_at`    | `Option<Instant>`| Wall-clock start; cleared on Idle                             |

The split between `hands_free_mode` (persistent) and `session_hands_free`
(transient) exists to fix a v2.x bug where a Fn-double-tap permanently
mutated the persistent setting until a manual reset. Read via
`effective_hands_free()`. Write only:

* `set_persistent_hands_free()` at the settings-load boundary.
* `enter_hands_free_session()` from shortcuts / tray entry points.

`set_state` clears `session_hands_free` and `recording_started_at` on every
Idle transition automatically — consumers don't have to remember to reset.

## Settings persistence

`settings/store.rs` keeps an in-memory 5 s TTL cache so `get_settings()` is
cheap to call from hot paths (`i18n::tr`, tray rebuild, every shortcut press).
Writes go through `write_settings_atomic` (temp file → fsync → atomic rename
→ `.bak` refresh AFTER) so a crash mid-write can never corrupt the live
settings file. The cache is refreshed-on-write and invalidated-on-reset.

## Licensing

`licensing/storage.rs` keeps an HMAC-signed cache of the user's TTP Pro
license. Per-machine secret in keychain; constant-time `verify_slice`
comparison; tamper invalidates the cache. The `LEGACY_HMAC_SECRET` from
v1.6.x is accepted as a fallback ONLY until `keychain::legacy_migration_complete`
fires (which happens on the first successful machine-signed save). After
that, a forged legacy file is rejected — closes the
"attacker extracts the binary constant and plants pre-install" vector.

Offline grace: 14 days since `last_validated_at`. After that, the cache
stops granting Pro until the next online validation succeeds. Tested via
`is_pro_at(record, now)` (pure function).

## i18n contract

Strings live in `src/i18n/locales/{en,fr}.json`. Rust modules that surface
strings to the user emit translation KEYS (e.g. `error.microphone_permission_denied`)
which the React side resolves via i18next. Two build-time checks enforce
the contract:

* `scripts/check-i18n-parity.mjs` — every leaf path exists in both EN and FR,
  and FR isn't accidentally the verbatim EN string (catches missed translations).
* `scripts/check-i18n-keys-used.mjs` — every `t('xxx.yyy')` callsite in TSX/TS
  source resolves against a real key in `en.json` (catches typos and dead refs).

Both run as part of `npm run build`.

## Recording → Transcription pipeline

`transcription/pipeline.rs::process_recording` orchestrates the whole
post-stop flow: validate WAV → backup → convert to 16 kHz mono → upload
to Groq Whisper → handle empty / hallucinated / oversized responses →
optionally call Groq LLM polish → apply user dictionary → paste (direct
typing for ≤2000 chars, clipboard fallback above).

Pure-function decision points are extracted as testable helpers:

* `classify_transcription_error(&str)` — Whisper API error → analytics
  category + HTTP status. 8 unit tests.
* `classify_polish_error(&str)` — same for the polish LLM. 4 unit tests.
* `is_hallucination(&str)` — Whisper hallu filter. Has its own test module
  with 10+ tests against curated French/English samples.
* `apply_dictionary_to_text(text, &entries)` — pure variant of
  `apply_dictionary` (which reads the global cache). 13 unit tests.

## Frontend architecture

* **Zustand store** (`src/stores/settings-store.ts`) is the single source
  of truth for user-visible state. Every window reads from it. Loads via
  `safeInvoke('get_settings')`; writes via `invoke('set_settings', { settings })`
  which the Rust side emits `settings-changed` for; the store re-syncs.
* **Tauri event hooks** (`src/hooks/use*.ts`) are thin subscribers. Each
  one calls `useTauriEvent` exactly once for its lifetime to avoid the
  re-subscription churn historically blamed for TTP-5.
* **Pure decision helpers** (`src/lib/updater-decisions.ts`,
  `src/lib/audio-stream-error.ts`, `src/lib/translateRustMessage.ts`,
  `src/lib/theme.ts`, `src/lib/pii-scrub.ts`) extract the testable logic
  out of the side-effect-heavy hooks. Most have ≥10 unit tests.

## Build / release

`npm run build` runs: i18n parity → i18n key-existence → `tsc --noEmit` →
`vite build`. The release pipeline (`.github/workflows/release.yml`) adds:
toolchain pinning (Rust 1.85.0 explicit), tag-vs-source version sanity
check, beta-tag regex (rejects unknown pre-release suffixes), Apple cert
import + notarization via App Store Connect API key (.p8), Windows cert
import + thumbprint inject into `tauri.conf.json` so signtool actually
runs, Sentry debug-symbol upload + JS sourcemap upload.

Updater bundles are minisign-signed; the public key is baked into
`tauri.conf.json` and verified on every installed client. Recovery
procedure is in `SECURITY.md`.

## Test surface

Total test count at v3.1 wave 4:

* Rust: licensing/HMAC roundtrip + tamper (8) · usage signing + trial day math (10) ·
  settings atomic write + cache TTL (5) · pipeline transcription classifier (8) ·
  pipeline polish classifier (4) · fnkey FSM (16) · apply_dictionary (13) ·
  backup validate + duration (3) · is_pro_at offline grace (7) · vad_step + required_ticks (9).
* Frontend (vitest + happy-dom): 88+ tests across translateRustMessage,
  safeInvoke, settings-store, updater-decisions, audio-stream-error,
  useTranscription, theme, pii-scrub, i18n config.

All Rust tests run under `cargo test --lib` (wired in `release.yml`). Run
the frontend suite with `npm run test:run`.
