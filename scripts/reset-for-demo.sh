#!/usr/bin/env bash
# TTP - Talk To Paste
# DEMO PREP — wipes all user data so the next launch behaves like a fresh
# install (onboarding wizard re-shows, usage stats reset, dictionary empty,
# history empty, trial counter resets) WITHOUT touching:
#   - the installed .app bundle (no need to re-install / re-grant Gatekeeper)
#   - macOS TCC permissions (Mic / Accessibility / Input Monitoring stay
#     granted — saves you the System Settings dance during the recording)
#
# Use this before recording a presentation video. Pair with QuickTime,
# CleanShot X, or Screen Studio (https://screen.studio) for output that
# looks like a Stripe / Linear product demo.
#
# Usage:
#   ./scripts/reset-for-demo.sh            # interactive, asks before each step
#   ./scripts/reset-for-demo.sh --yes      # no prompts
#   ./scripts/reset-for-demo.sh --dry-run  # preview only

set -uo pipefail

BUNDLE_ID="com.ttp.desktop"
CONFIG_NAME="ttp"

DRY_RUN=0
ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    --dry-run|-n) DRY_RUN=1 ;;
    --yes|-y)     ASSUME_YES=1 ;;
    --help|-h)    grep '^#' "$0" | head -16; exit 0 ;;
  esac
done

c_green()  { printf '\033[32m%s\033[0m' "$1"; }
c_yellow() { printf '\033[33m%s\033[0m' "$1"; }
c_dim()    { printf '\033[2m%s\033[0m' "$1"; }

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf '  %s %s\n' "$(c_yellow '[dry-run]')" "$*"
  else
    eval "$@" 2>/dev/null || true
  fi
}

ask() {
  [ "$ASSUME_YES" = "1" ] && return 0
  [ "$DRY_RUN" = "1" ] && return 0
  printf '  %s [y/N] ' "$1"
  read -r reply
  [[ "$reply" =~ ^[Yy]$ ]]
}

header() { printf '\n%s\n' "$(c_green "▸ $1")"; }
note()   { printf '  %s\n' "$(c_dim "$1")"; }

if [ "$(uname)" != "Darwin" ]; then
  echo "macOS only."; exit 1
fi

printf '%s\n' "$(c_green '🎬 TTP demo reset')"
note "Mode: $([ "$DRY_RUN" = 1 ] && echo 'DRY RUN' || echo 'LIVE')"
note "Keeps the .app and TCC permissions; only wipes user data."

# 1. Quit TTP so file writes don't race the reset.
header "1/4  Quit TTP if running"
if pgrep -fi "TTP" >/dev/null; then
  if ask "TTP is running. Quit it?"; then
    run "pkill -fi 'TTP by AmirKS' || pkill -fi 'ttp'"
  fi
else
  note "Not running."
fi

# 2. Wipe the config dir (settings.json, usage.json, dictionary, history,
#    license, last_seen_version → causes the wizard + "What's New" to re-show).
header "2/4  Wipe user data (~/Library/Application Support/$CONFIG_NAME)"
CONFIG_DIR="$HOME/Library/Application Support/$CONFIG_NAME"
if [ -d "$CONFIG_DIR" ]; then
  if ask "Delete every file in $CONFIG_DIR ?"; then
    run "rm -rf \"$CONFIG_DIR\""
    note "Wiped. The wizard will re-show on next launch."
  fi
else
  note "Already empty."
fi

# 3. Tauri runtime state (window position / size, WebKit localStorage —
#    the localStorage carries the tutorial-pill-dismissed flag).
header "3/4  Wipe Tauri runtime state and WebKit storage"
for path in \
  "$HOME/Library/Application Support/$BUNDLE_ID" \
  "$HOME/Library/Caches/$BUNDLE_ID" \
  "$HOME/Library/WebKit/$BUNDLE_ID" \
  "$HOME/Library/HTTPStorages/$BUNDLE_ID" \
  "$HOME/Library/HTTPStorages/$BUNDLE_ID.binarycookies" \
  "$HOME/Library/Saved Application State/$BUNDLE_ID.savedState" \
; do
  if [ -e "$path" ]; then
    run "rm -rf \"$path\""
    note "Removed: $(c_dim "$path")"
  fi
done

# 4. Keychain — wipe ONLY the Groq API key + usage HMAC. Keep the license
#    HMAC so if you re-activate a Pro key during the demo, it survives.
#    (If you want to demo trial onboarding, also remove license_hmac_secret.)
header "4/4  Wipe Keychain entries (Groq key + usage HMAC)"
if ask "Delete Groq API key and usage HMAC from Keychain?"; then
  run "security delete-generic-password -s '$BUNDLE_ID' -a 'groq_api_key' >/dev/null"
  run "security delete-generic-password -s '$BUNDLE_ID' -a 'usage_hmac_secret' >/dev/null"
  note "Done. License HMAC (license_hmac_secret) preserved."
  note "To also wipe license: security delete-generic-password -s '$BUNDLE_ID' -a 'license_hmac_secret'"
fi

# --- Done --------------------------------------------------------------------
echo
if [ "$DRY_RUN" = "1" ]; then
  printf '%s %s\n' "$(c_yellow '✔')" "Dry run done."
else
  printf '%s %s\n' "$(c_green '✔')" "Ready for the demo. Launch TTP and the onboarding wizard will re-appear."
  note "TCC permissions are still granted, so you can record the first dictation immediately."
fi
