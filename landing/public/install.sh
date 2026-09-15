#!/bin/sh
# TTP - Talk To Paste installer
# Usage: curl -fsSL ttp.amirks.eu/install.sh | sh
#
# Strategy: query the GitHub releases LIST endpoint (not /releases/latest —
# that one excludes prereleases, and our beta tags like v3.1.6-1 are
# published as prereleases). The list is newest-first, so the first asset
# matching our platform pattern is always the actual newest build, stable
# or beta. This mirrors landing/src/lib/github.ts, which the website's own
# download buttons already use for the same reason: v3.0.4 was the last
# bare-stable tag, so /releases/latest was stuck pointing at it while every
# subsequent v3.1.x shipped as a numbered prerelease.
set -e

REPO="AmirK-S/TTP"
API="https://api.github.com/repos/$REPO/releases"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  PATTERN='_aarch64\.dmg' ;;
  Darwin-x86_64) PATTERN='_x64\.dmg' ;;
  Darwin-*)      echo "Error: unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
  *)             echo "Error: this installer is macOS-only. Download Windows builds from https://github.com/$REPO/releases" >&2; exit 1 ;;
esac

echo "Resolving latest release..."
URL=$(curl -fsSL "$API" \
  | grep -oE '"browser_download_url": *"[^"]+'"$PATTERN"'"' \
  | head -1 \
  | sed -E 's/.*"browser_download_url": *"([^"]+)".*/\1/')

if [ -z "$URL" ]; then
  echo "Error: could not find a macOS build in the latest release." >&2
  echo "Please download manually from https://github.com/$REPO/releases" >&2
  exit 1
fi

# Refuse to install if /Applications isn't writable rather than failing midway.
if [ ! -w /Applications ]; then
  echo "Error: /Applications isn't writable by this user." >&2
  echo "Re-run with: curl -fsSL ttp.amirks.eu/install.sh | sudo sh" >&2
  exit 1
fi

DMG="/tmp/TTP-installer-$$.dmg"
MOUNT=""
cleanup() {
  if [ -n "$MOUNT" ] && [ -d "$MOUNT" ]; then
    hdiutil detach "$MOUNT" -quiet 2>/dev/null \
      || hdiutil detach "$MOUNT" -force -quiet 2>/dev/null \
      || true
  fi
  rm -f "$DMG"
}
trap cleanup EXIT

echo "Downloading $URL..."
curl -fSL "$URL" -o "$DMG"

# Sanity-check we got an actual DMG (catches HTML error pages from proxies/captive portals).
if ! hdiutil imageinfo "$DMG" >/dev/null 2>&1; then
  SIZE=$(stat -f%z "$DMG" 2>/dev/null || echo "?")
  echo "Error: download is not a valid disk image (size=$SIZE bytes)." >&2
  exit 1
fi

echo "Mounting..."
# Use plist output so we can extract the mount-point reliably even if the
# volume name has spaces or matches an existing /Volumes/TTP* directory.
PLIST=$(hdiutil attach "$DMG" -nobrowse -noautoopen -plist)
MOUNT=$(printf '%s' "$PLIST" | /usr/bin/python3 -c '
import sys, plistlib
d = plistlib.loads(sys.stdin.buffer.read())
for e in d.get("system-entities", []):
    mp = e.get("mount-point")
    if mp:
        print(mp)
        break
' 2>/dev/null || true)

if [ -z "$MOUNT" ] || [ ! -d "$MOUNT" ]; then
  # Fallback: glob /Volumes/TTP* (least recently created last; we just mounted, so newest wins).
  MOUNT=$(ls -dt /Volumes/TTP* 2>/dev/null | head -1)
fi
if [ -z "$MOUNT" ] || [ ! -d "$MOUNT" ]; then
  echo "Error: could not locate the mounted TTP volume." >&2
  exit 1
fi

APP_PATH=$(find "$MOUNT" -maxdepth 1 -name "*.app" -print 2>/dev/null | head -1)
if [ -z "$APP_PATH" ]; then
  echo "Error: no .app bundle inside the DMG." >&2
  exit 1
fi
APP_NAME=$(basename "$APP_PATH")
DEST="/Applications/$APP_NAME"

# Quit a running copy so cp -R doesn't corrupt the bundle AND so the new
# instance can grab its own Input Monitoring / Accessibility privileges
# cleanly. Always attempt — AppleScript's `quit` is a no-op if the app
# isn't running. We don't gate on pgrep first, because pgrep -f pattern
# matching can miss the running process depending on shell quoting and
# leave the old binary in place.
echo "Closing TTP if running..."
APP_NAME_NO_EXT="${APP_NAME%.app}"
osascript -e "tell application \"$APP_NAME_NO_EXT\" to quit" >/dev/null 2>&1 || true

# Wait up to 5 seconds for a graceful quit, polling twice a second.
for _ in 1 2 3 4 5 6 7 8 9 10; do
  pgrep -f "/Applications/$APP_NAME/Contents/" >/dev/null 2>&1 || break
  sleep 0.5
done

# Belt-and-suspenders: force-kill anything still running. `pkill -f`
# matches against the full command line so it catches the TTP binary
# regardless of how it was launched.
pkill -f "/Applications/$APP_NAME/Contents/" 2>/dev/null || true
sleep 0.5

echo "Installing $APP_NAME..."
rm -rf "$DEST"
cp -R "$APP_PATH" /Applications/

# Strip Gatekeeper's quarantine attribute so unsigned/un-notarized builds
# launch on first run. Becomes a no-op once notarization is wired up.
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true

echo "Launching..."
open "$DEST"
echo "Done! TTP is ready."
