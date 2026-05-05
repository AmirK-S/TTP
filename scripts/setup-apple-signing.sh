#!/usr/bin/env bash
# One-shot setup for Apple code-signing + notarization on the GitHub Actions
# release workflow. Reads a .p12 you exported from Keychain Access, derives
# everything it can, prompts for the bits it can't, and uploads all 7 secrets
# to the GitHub repo via `gh`.
#
# Usage:
#   ./scripts/setup-apple-signing.sh [path/to/cert.p12]
#
# Prerequisites you must do manually before running:
#   1. Have a "Developer ID Application: ..." identity in Keychain Access.
#      (Generate CSR → upload to developer.apple.com → download .cer → import.)
#   2. Right-click that identity in Keychain Access → Export → save as .p12,
#      choose any password (you'll paste it here).
#   3. Generate an app-specific password at https://appleid.apple.com →
#      Sign-In and Security → App-Specific Passwords. Label it "TTP notarization".
#   4. `gh auth login` so this script can push secrets.
set -euo pipefail

REPO="AmirK-S/TTP"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
ok()   { printf '\033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[33m!\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[31m✗\033[0m %s\n' "$*" >&2; exit 1; }

bold "TTP Apple signing setup → $REPO"
echo

# ── Tooling checks ─────────────────────────────────────────────────────────
command -v gh      >/dev/null || die "gh CLI is not installed. Install with: brew install gh"
command -v openssl >/dev/null || die "openssl is not installed."
gh auth status >/dev/null 2>&1 || die "gh is not authenticated. Run: gh auth login"

# ── If no Developer ID cert exists yet, walk the user through making one ──
HAS_DEV_ID=$(security find-identity -v -p codesigning 2>/dev/null | grep -c "Developer ID Application" || true)
if [ "${1:-}" = "" ] && [ "$HAS_DEV_ID" -eq 0 ]; then
  cat <<'GUIDE'
No "Developer ID Application" identity found in your keychain.

You need to create one before running this script:

  1. Open Keychain Access.app → menu Certificate Assistant →
     "Request a Certificate from a Certificate Authority..."
       • User Email Address: your Apple-ID email
       • Common Name:        anything (e.g. "TTP Developer ID")
       • CA Email Address:   leave blank
       • Choose: "Saved to disk" + "Let me specify key pair information"
       • Key Size 2048, Algorithm RSA → Continue → save the .certSigningRequest

  2. Go to https://developer.apple.com/account/resources/certificates/add
       • Choose "Developer ID Application" (under "Software")
       • Upload the .certSigningRequest from step 1
       • Download the resulting .cer file

  3. Double-click the .cer to import it into your login keychain.
     Verify with:
       security find-identity -v -p codesigning
     You should see one line like:
       1) ABCDEF1234... "Developer ID Application: Your Name (TEAMID)"

  4. Right-click the identity in Keychain Access → Export →
     File Format: Personal Information Exchange (.p12) → save somewhere
     (any password you like; you'll paste it here next time).

  5. Re-run this script:
       ./scripts/setup-apple-signing.sh path/to/cert.p12

You also need (collect now, you'll paste them when prompted):
  • App-specific password from https://appleid.apple.com → Sign-In and
    Security → App-Specific Passwords. Label it "TTP notarization".
GUIDE
  exit 0
fi

# ── Locate .p12 ────────────────────────────────────────────────────────────
P12="${1:-}"
if [ -z "$P12" ]; then
  # Check common locations in the repo / current dir.
  for candidate in ./cert.p12 ./apple-cert.p12 ./DeveloperID.p12 ~/Downloads/cert.p12 ~/Downloads/Certificates.p12; do
    if [ -f "$candidate" ]; then P12="$candidate"; break; fi
  done
fi
if [ -z "$P12" ] || [ ! -f "$P12" ]; then
  echo "Need the path to your exported .p12 file."
  echo "Export it from Keychain Access (right-click identity → Export → .p12)."
  read -r -p ".p12 path: " P12
  [ -f "$P12" ] || die "File not found: $P12"
fi
ok "Using certificate: $P12"

# ── Read .p12 password (the one you typed in Keychain Access) ──────────────
read -r -s -p "Password you set on the .p12 export: " P12_PASS; echo
[ -n "$P12_PASS" ] || die "Empty password."

# Verify password works and extract identity info.
CERT_PEM=$(openssl pkcs12 -in "$P12" -nokeys -clcerts -passin "pass:$P12_PASS" 2>/dev/null) \
  || die "Could not open .p12 with that password."

SUBJECT=$(printf '%s' "$CERT_PEM" | openssl x509 -noout -subject 2>/dev/null || true)
[ -n "$SUBJECT" ] || die "No certificate found in .p12."

# Identity name = the CN (e.g. "Developer ID Application: Foo Bar (TEAMID)").
SIGNING_IDENTITY=$(printf '%s' "$SUBJECT" | sed -nE 's/.*CN[[:space:]]*=[[:space:]]*([^/,]+).*/\1/p' | sed 's/[[:space:]]*$//')
[ -n "$SIGNING_IDENTITY" ] || die "Could not parse CN from cert subject: $SUBJECT"
case "$SIGNING_IDENTITY" in
  "Developer ID Application:"*) : ;;
  *) warn "CN is '$SIGNING_IDENTITY' — expected 'Developer ID Application: ...'. Continuing anyway." ;;
esac

# Team ID = the parenthetical inside CN, also matches OU.
TEAM_ID=$(printf '%s' "$SIGNING_IDENTITY" | sed -nE 's/.*\(([A-Z0-9]{10})\).*/\1/p')
if [ -z "$TEAM_ID" ]; then
  TEAM_ID=$(printf '%s' "$SUBJECT" | sed -nE 's/.*OU[[:space:]]*=[[:space:]]*([A-Z0-9]{10}).*/\1/p')
fi
[ -n "$TEAM_ID" ] || die "Could not extract 10-character Team ID from cert."

ok "Signing identity: $SIGNING_IDENTITY"
ok "Team ID:          $TEAM_ID"

# ── Apple ID + app-specific password ───────────────────────────────────────
DEFAULT_APPLE_ID="${APPLE_ID:-amirksmain@gmail.com}"
read -r -p "Apple ID email [$DEFAULT_APPLE_ID]: " APPLE_ID_INPUT
APPLE_ID_FINAL="${APPLE_ID_INPUT:-$DEFAULT_APPLE_ID}"

echo "App-specific password from https://appleid.apple.com (format: xxxx-xxxx-xxxx-xxxx)"
read -r -s -p "App-specific password: " APPLE_APP_PASS; echo
[ -n "$APPLE_APP_PASS" ] || die "Empty app-specific password."

# ── Random keychain password for the CI runner's temp keychain ─────────────
KEYCHAIN_PASS=$(openssl rand -base64 24 | tr -d '\n=/+' | cut -c1-24)

# ── Base64 the .p12 for the secret ─────────────────────────────────────────
P12_B64=$(base64 -i "$P12")

# ── Confirm before pushing ─────────────────────────────────────────────────
echo
bold "Will set the following secrets on github.com/$REPO:"
echo "  APPLE_CERTIFICATE              $(printf '%s' "$P12_B64" | wc -c | tr -d ' ') bytes (base64)"
echo "  APPLE_CERTIFICATE_PASSWORD     [hidden]"
echo "  KEYCHAIN_PASSWORD              [random, $KEYCHAIN_PASS first 4: ${KEYCHAIN_PASS:0:4}…]"
echo "  APPLE_SIGNING_IDENTITY         $SIGNING_IDENTITY"
echo "  APPLE_ID                       $APPLE_ID_FINAL"
echo "  APPLE_PASSWORD                 [hidden]"
echo "  APPLE_TEAM_ID                  $TEAM_ID"
echo
read -r -p "Proceed? [y/N] " CONFIRM
case "$CONFIRM" in
  y|Y|yes|YES) : ;;
  *) die "Aborted." ;;
esac

# ── Push secrets ───────────────────────────────────────────────────────────
# IMPORTANT: use --body-file with a temp file rather than `--body -` piped
# from stdin. The piped form silently truncates large or odd-shaped values
# (observed 2026-05-05 — a 4380-char base64 cert ended up stored as a single
# newline char, and short ASCII values got reduced to 1 char too).
set_secret() {
  local name="$1" value="$2"
  local tmp
  tmp=$(mktemp)
  printf '%s' "$value" > "$tmp"
  gh secret set "$name" --repo "$REPO" --body-file "$tmp" >/dev/null
  rm -f "$tmp"
  ok "$name"
}

set_secret APPLE_CERTIFICATE          "$P12_B64"
set_secret APPLE_CERTIFICATE_PASSWORD "$P12_PASS"
set_secret KEYCHAIN_PASSWORD          "$KEYCHAIN_PASS"
set_secret APPLE_SIGNING_IDENTITY     "$SIGNING_IDENTITY"
set_secret APPLE_ID                   "$APPLE_ID_FINAL"
set_secret APPLE_PASSWORD             "$APPLE_APP_PASS"
set_secret APPLE_TEAM_ID              "$TEAM_ID"

# ── Clean up sensitive material ────────────────────────────────────────────
unset P12_PASS APPLE_APP_PASS P12_B64 KEYCHAIN_PASS

echo
bold "Done."
echo "Cut a tagged release to verify signing+notarization works:"
echo "  git tag v1.6.1 && git push origin v1.6.1"
echo
echo "Watch the run at: https://github.com/$REPO/actions"
echo "If notarization succeeds, the resulting DMG launches without Gatekeeper warnings."
echo
warn "The .p12 ($P12) still exists on disk. Delete it now if you don't need it:"
echo "  rm -P \"$P12\""
