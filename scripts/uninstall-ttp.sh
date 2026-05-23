#!/usr/bin/env bash
# TTP - Talk To Paste
# Complete uninstaller. Removes the .app, every config file, the keychain
# entries (Groq key + HMAC secrets), the LaunchAgent, and resets the macOS
# TCC permissions so a fresh install will re-prompt for Microphone /
# Accessibility / Input Monitoring as if it were the first time.
#
# Usage:
#   ./scripts/uninstall-ttp.sh           # interactive, asks before each step
#   ./scripts/uninstall-ttp.sh --yes     # no prompts, nukes everything
#   ./scripts/uninstall-ttp.sh --dry-run # show what would be removed, do nothing

set -uo pipefail

BUNDLE_ID="com.ttp.desktop"
APP_NAME="TTP by AmirKS"
CONFIG_NAME="ttp"

DRY_RUN=0
ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    --dry-run|-n) DRY_RUN=1 ;;
    --yes|-y)     ASSUME_YES=1 ;;
    --help|-h)
      grep '^#' "$0" | head -20
      exit 0
      ;;
  esac
done

c_red()    { printf '\033[31m%s\033[0m' "$1"; }
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

# --- 0. Bail if not on macOS -------------------------------------------------
if [ "$(uname)" != "Darwin" ]; then
  echo "This script is macOS-only. For Linux/Windows, remove the bundle and config dir manually."
  exit 1
fi

printf '%s\n' "$(c_red '⚠  TTP complete uninstaller')"
note "Mode: $([ "$DRY_RUN" = 1 ] && echo 'DRY RUN — no changes will be made' || echo 'LIVE — files will be deleted')"
note "Bundle ID: $BUNDLE_ID"

# --- 1. Quit any running TTP process -----------------------------------------
header "1/7  Quit running TTP processes"
if pgrep -fi "TTP" >/dev/null; then
  if ask "TTP is running. Quit it?"; then
    run "pkill -fi 'TTP by AmirKS' || pkill -fi 'ttp'"
    note "Killed."
  fi
else
  note "Not running."
fi

# --- 2. Remove the .app from /Applications -----------------------------------
header "2/7  Remove the .app bundle"
APP_PATH="/Applications/$APP_NAME.app"
if [ -d "$APP_PATH" ]; then
  if ask "Delete $APP_PATH ?"; then
    run "rm -rf \"$APP_PATH\""
    note "Removed."
  fi
else
  note "Not present in /Applications."
fi

# --- 3. Config files (settings, usage, dictionary, history, license, etc.) ---
header "3/7  Remove config directory"
CONFIG_DIR="$HOME/Library/Application Support/$CONFIG_NAME"
if [ -d "$CONFIG_DIR" ]; then
  if ask "Delete $CONFIG_DIR ?"; then
    run "rm -rf \"$CONFIG_DIR\""
    note "Removed."
  fi
else
  note "Not present."
fi

# --- 4. Tauri / WebKit caches + bundle identifier dir ------------------------
header "4/7  Remove WebKit caches and Tauri runtime data"
for path in \
  "$HOME/Library/Application Support/$BUNDLE_ID" \
  "$HOME/Library/Caches/$BUNDLE_ID" \
  "$HOME/Library/WebKit/$BUNDLE_ID" \
  "$HOME/Library/HTTPStorages/$BUNDLE_ID" \
  "$HOME/Library/HTTPStorages/$BUNDLE_ID.binarycookies" \
  "$HOME/Library/Preferences/$BUNDLE_ID.plist" \
  "$HOME/Library/Saved Application State/$BUNDLE_ID.savedState" \
; do
  if [ -e "$path" ]; then
    run "rm -rf \"$path\""
    note "Removed: $(c_dim "$path")"
  fi
done

# --- 5. LaunchAgent (autostart) ----------------------------------------------
header "5/7  Remove LaunchAgent (autostart)"
for plist in "$HOME/Library/LaunchAgents/$BUNDLE_ID.plist" \
             "$HOME/Library/LaunchAgents/$APP_NAME.plist"; do
  if [ -f "$plist" ]; then
    run "launchctl unload \"$plist\""
    run "rm -f \"$plist\""
    note "Removed: $(c_dim "$plist")"
  fi
done

# --- 6. macOS Keychain entries (Groq key + HMAC secrets) ---------------------
header "6/7  Remove Keychain entries"
note "Service: $BUNDLE_ID  (3 entries: groq_api_key, license_hmac_secret, usage_hmac_secret)"
for account in groq_api_key license_hmac_secret usage_hmac_secret; do
  run "security delete-generic-password -s '$BUNDLE_ID' -a '$account' >/dev/null"
  note "Deleted: $(c_dim "$account")"
done

# --- 7. macOS TCC permissions (privacy database) -----------------------------
header "7/7  Reset macOS TCC permissions"
note "Microphone, Accessibility, Input Monitoring will re-prompt on next install."
if ask "Reset TCC permissions for $BUNDLE_ID ?"; then
  for service in Microphone Accessibility ListenEvent PostEvent; do
    run "tccutil reset $service $BUNDLE_ID >/dev/null"
    note "Reset: $(c_dim "$service")"
  done
fi

# --- Done --------------------------------------------------------------------
echo
if [ "$DRY_RUN" = "1" ]; then
  printf '%s %s\n' "$(c_yellow '✔')" "Dry run complete. Re-run without --dry-run to actually uninstall."
else
  printf '%s %s\n' "$(c_green '✔')" "TTP completely removed. A fresh install will behave like a first launch."
fi
