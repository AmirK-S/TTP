# Threat model

This document records the threats TTP is designed to resist, the threats it
explicitly does not resist, and the assumptions behind those choices. It exists
so security reports can be evaluated against an explicit baseline rather than
"is this an issue or not?", and so contributors don't accidentally introduce
fixes that pretend to defend against threats outside the scope.

Last updated: 2026-06-10.

## In scope

TTP defends against the following attacker capabilities.

### 1. Network-level attackers between the user's Mac and Groq / Lemon Squeezy

- The HTTP client uses `rustls-tls` (no OpenSSL). TLS 1.2+ with platform root
  trust.
- API keys are sent via the `Authorization: Bearer` header only over HTTPS.
- License-validation calls include only the user-supplied license key + a
  per-machine `instance_id`. No PII is sent.

### 2. Other processes on the same Mac, running as the same user

- The Groq API key is stored in the OS keychain (`security-tool` accessible).
  TTP never writes it to disk in plain text.
- Per-machine HMAC secrets for license + usage caches are stored in the same
  keychain. License files signed on one machine cannot be replayed on another.
- Plain-text logs in `~/Library/Application Support/com.ttp.desktop/ttp.log`
  are scrubbed of API keys, file paths, emails, and bearer tokens via regex
  before write. Sentry breadcrumbs go through an equivalent JS scrubber on the
  frontend (`src/lib/sentry.ts::scrubMessage`).
- Transcript content is NEVER written to stderr or to the persistent log file.
  The hallucination filter, dictionary classifier, and pipeline progress
  emitters log redacted character counts only.

### 3. Forged / tampered local state

- `license.json` and `usage.json` are HMAC-signed with a per-machine secret.
  An attacker who flips fields with a text editor invalidates the signature
  and the file is treated as missing on next load.
- Constant-time HMAC verification (`verify_slice`) defeats first-byte
  timing attacks on the comparator.
- The trial counter (`trial_count`) is signed and load-bearing: once set
  to 1, no re-init grants a fresh trial via "delete the file and relaunch"
  on the same machine.

### 4. Renderer (webview) compromise

- The `process_audio` Tauri command canonicalizes its `audio_path` argument
  and verifies it lives under the app's recordings directory. A compromised
  renderer cannot ask the backend to upload `~/.ssh/id_ed25519` to Groq.
- The HTTP capability allowlist is scoped tightly (Groq, Sentry ingest).
  Renderer cannot freely fetch arbitrary URLs.
- All Tauri commands either take primitive arguments or perform their own
  validation; the renderer cannot pass an `AppHandle` or otherwise escalate.

### 5. Supply chain (release pipeline)

- Cargo.lock is committed; the toolchain is pinned (`rust-toolchain.toml`
  via `.github/workflows/release.yml`).
- Tag-vs-source version check fails the build if `git tag` doesn't match
  `package.json` + `Cargo.toml`.
- macOS Developer ID signing + notarization via App Store Connect API key
  (.p8); Windows code signing via PFX with thumbprint wired into Tauri
  bundler at build time.
- Updater bundles are minisign-signed; the public key is baked into
  `tauri.conf.json` and verified by every installed client.

## Out of scope (intentionally)

The following threats are NOT defended against. If a report focuses on these,
the answer is "intentional design choice, here's why."

### A. Attacker with arbitrary code execution as the user

Once you can run code as the user that owns the TTP process:

- You can read the keychain entries TTP wrote (Keychain ACLs limit *which*
  binaries can read each entry, but only at the granularity macOS provides).
- You can read `license.json` + `usage.json` + the keychain secret used to
  re-sign them, so you can forge a `Pro` record locally.
- You can attach to the live process and read the in-memory API key.
- You can replace the TTP binary on disk with a malicious version.

This is the standard OS security boundary. We do not try to defend against
it via code obfuscation, anti-debug tricks, or cert pinning — it would not
work and would punish legitimate users (security-tool users, corporate AV
inspectors, MDM proxies).

### B. Cert pinning vs MITM via user-installed root

`reqwest` trusts the OS root store. A user who has installed a corporate
proxy CA (Zscaler / Bluecoat / parental control) sees their TLS handshake
intercepted. We do NOT pin certs:

- Pinning would brick the app whenever Cloudflare / Let's Encrypt rotate
  the Groq / Lemon Squeezy / GitHub Releases certificate chain.
- An attacker who can install a system root on the user's Mac can already
  read the keychain via approach A, so pinning gains nothing security-wise.

Users in hostile network environments should treat any voice-to-text app
as a confidentiality risk regardless of pinning.

### C. Trial reset via uninstall + reinstall

The v2.1.4 uninstaller wipes the keychain HMAC secret + usage.json. After
reinstall, the new install sees no signed trial record and starts a fresh
4-day trial.

This is documented in [[project_trial_reset_loophole]]. We accept it as a
friction-only moat. The cost of closing the loophole (require account
signup at install, or call out to a server-side fingerprint) would push
legitimate users into a flow we have explicitly chosen not to build
("free, no signup, no credit card").

### D. macOS Accessibility API misuse

`enigo` (used for paste/typing) requires Accessibility permission, which
the user grants explicitly. With that permission, TTP can in principle
type or read anywhere. We rely on:

- The user granting permission consciously (TTP shows a guided onboarding
  step explaining what Accessibility is used for).
- The Tauri commands using Accessibility only inside the post-transcription
  paste pipeline (`pipeline.rs::process_recording`), never in response to
  free-form renderer requests.

If a future feature wants Accessibility for a different purpose, document
it in the onboarding copy first.

### E. Third-party services TTP depends on

Groq, Lemon Squeezy, Sentry, GitHub Releases each have their own threat
model. TTP does not duplicate their defenses. See SECURITY.md scope section.

## Open questions

These are decisions the project has not yet committed to one way or the other.

- **`LEGACY_HMAC_SECRET` lifetime**: the constant is in the binary, so an
  attacker who reads `ttp.app` can sign their own legacy-format `license.json`
  and plant it pre-install. Mitigation: track `legacy_migration_complete` per
  account in keychain (see `src-tauri/src/licensing/storage.rs`); once any
  record has been re-signed with the machine secret, reject ValidLegacy.
  Wave 2 work, not yet shipped.

- **`history.json` encryption at rest**: up to 500 transcript entries are
  stored as plaintext JSON. Any process running as the user can read them.
  Open question: encrypt with a per-machine keychain-stored key, or accept
  that approach A makes encryption moot? Current answer: ship a "Save
  history" toggle (already present) so privacy-conscious users can disable.
