# Security Policy

We take the security of TTP (Talk To Paste) seriously. Thanks for taking the time to disclose responsibly.

## Supported Versions

Only the latest minor of the v1.x line receives security fixes. Older releases are not patched — please update before reporting an issue you can only reproduce on an older build.

| Version          | Supported          |
| ---------------- | ------------------ |
| v1.x (latest minor) | Yes             |
| v1.x (older minors) | No              |
| < v1.0           | No                 |

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
- **Aptabase** (opt-in usage analytics)
- **GitHub** (release hosting / updater feed)

Reports about third-party infrastructure misconfiguration on TTP's side (e.g. a leaked key, a misconfigured CSP, a missing signature check) are in scope. Reports about the third-party service itself (e.g. a Groq API bug) are not.

## Bounty

There is no formal bug bounty program at this stage. Valid reports will be credited in the `CHANGELOG` and in a hall-of-fame section once the first report lands — assuming you want public credit.

Thanks for helping keep TTP and its users safe.
