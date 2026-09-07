# Security Policy

We take the security of TTP (Talk To Paste) seriously. Thanks for taking the time to disclose responsibly.

## Supported Versions

Only the latest minor of the current major line receives security fixes. Older releases are not patched, please update before reporting an issue you can only reproduce on an older build.

| Version                | Supported |
| ---------------------- | --------- |
| v3.x (latest minor)    | Yes       |
| v3.x (older minors)    | No        |
| < v3.0                 | No        |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security reports.**

Email vulnerabilities privately to:

**`amirksmain+security@gmail.com`**

Use a clear subject line, e.g. `[TTP Security] <short description>`. Include:

- A description of the issue and its impact.
- Step-by-step reproduction instructions (and a PoC if you have one).
- The TTP version, OS, and any relevant environment details.
- Whether you'd like to be credited (and how) if the report is valid.

You should expect an acknowledgement within **5 business days**. We'll keep you updated as we triage, fix, and ship a release. Please give us a reasonable window to patch before any public disclosure.

### PGP

There is no published PGP key today. If you'd like to encrypt your report, mention it in the first email and one will be provided on request.

## Scope

**In scope**

- The TTP desktop app — Rust backend (`src-tauri/`) and React frontend (`src/`).
- The TTP landing page (`landing/`, served at `https://ttp.amirks.eu`).
- The release/update pipeline (signed installers, minisign-verified updater payloads).

**Out of scope**

The following are operated by third parties and have their own vulnerability disclosure programs. Please report to them directly:

- **Groq** (transcription + LLM API)
- **Lemon Squeezy** (licensing / checkout)
- **Sentry** (opt-in crash reporting)
- **GitHub** (release hosting / updater feed)

Reports about third-party infrastructure misconfiguration on TTP's side (e.g. a leaked key, a misconfigured CSP, a missing signature check) are in scope. Reports about the third-party service itself (e.g. a Groq API bug) are not.

## Bounty

There is no formal bug bounty program at this stage. Valid reports will be credited in the `CHANGELOG` and in a hall-of-fame section once the first report lands — assuming you want public credit.

Thanks for helping keep TTP and its users safe.

## Release-signing key recovery

TTP's auto-updater verifies every downloaded bundle against a minisign public
key baked into `src-tauri/tauri.conf.json` (the `plugins.updater.pubkey`
field). The matching **private** key signs every release artifact and is
currently held in two places:

1. `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` GitHub
   Actions secrets, used by `.github/workflows/release.yml`. GitHub stores
   secrets write-only, so they cannot be recovered from the repository UI.
2. An **offline copy** of the same private key + passphrase, stored in the
   maintainer's password manager (1Password, Bitwarden, or a hardware
   security device).

If the offline copy is lost AND the GitHub secret is rotated or wiped,
**all installed users lose auto-updates forever**, because the public key
baked into their on-disk `tauri.conf.json` no longer matches any signing
key we hold. Users would have to manually download + install every future
version.

### Rotating the key (planned)

1. Generate a new keypair locally:
   ```bash
   npx @tauri-apps/cli@latest signer generate -w ~/.tauri/ttp.key
   ```
   The command prints the public key on stdout and writes the password-
   protected private key to `~/.tauri/ttp.key`.
2. Store the private key + passphrase BOTH in:
   - the maintainer's password manager (offline, encrypted at rest), AND
   - the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret.
3. Replace the `pubkey` field in `src-tauri/tauri.conf.json` with the new
   public key.
4. **Important:** at least one transition release must be signed with BOTH
   the old and new key (or shipped with a manual-update note) so existing
   users can verify the bundle that swaps in the new pubkey. Without this,
   every installed v3.x user must do a fresh manual install of the post-
   rotation version.
5. Once the transition release is widely adopted (measured via the next
   minor's release-uptake telemetry), the old key can be retired.

### Emergency recovery

If the GitHub secret is wiped but the offline copy survives:
- Restore the offline private key + passphrase into the GitHub Actions
  secret with `gh secret set TAURI_SIGNING_PRIVATE_KEY`.

If BOTH copies are lost:
- The on-disk `pubkey` in `tauri.conf.json` cannot be matched by any new
  key. Cut a release with a fresh keypair AND announce that all users must
  manually re-install from the website to receive future updates. There is
  no shortcut.

This section is the recovery playbook. Keep it current when you rotate the
key.
