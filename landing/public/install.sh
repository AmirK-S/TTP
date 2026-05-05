#!/bin/sh
# TTP - Talk To Paste installer
# Usage: curl -fsSL ttp.amirks.eu/install.sh | sh
set -e

REPO="AmirK-S/TTP"
API="https://api.github.com/repos/$REPO/releases/latest"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  PATTERN='_aarch64\.dmg' ;;
  Darwin-x86_64) PATTERN='_x64\.dmg' ;;
  Darwin-*)      echo "Error: unsupported macOS architecture: $(uname -m)"; exit 1 ;;
  *)             echo "Error: this installer is macOS-only. Download Windows builds from https://github.com/$REPO/releases/latest"; exit 1 ;;
esac

echo "Finding latest TTP release..."
URL=$(curl -fsSL "$API" \
  | grep -oE '"browser_download_url": *"[^"]+'"$PATTERN"'"' \
  | head -1 \
  | sed -E 's/.*"browser_download_url": *"([^"]+)".*/\1/')

if [ -z "$URL" ]; then
  echo "Error: no DMG matching '$PATTERN' found in latest release."
  echo "Check https://github.com/$REPO/releases/latest"
  exit 1
fi

DMG="/tmp/TTP-installer-$$.dmg"
MOUNT=""
cleanup() {
  [ -n "$MOUNT" ] && [ -d "$MOUNT" ] && hdiutil detach "$MOUNT" -quiet 2>/dev/null || true
  rm -f "$DMG"
}
trap cleanup EXIT

echo "Downloading $URL..."
curl -fSL "$URL" -o "$DMG"

echo "Mounting..."
hdiutil attach "$DMG" -nobrowse -quiet

# Tauri DMGs mount under /Volumes/<productName>; pick the most recent TTP volume.
MOUNT=$(ls -dt /Volumes/TTP* 2>/dev/null | head -1)
if [ -z "$MOUNT" ] || [ ! -d "$MOUNT" ]; then
  echo "Error: could not locate the mounted TTP volume."
  exit 1
fi

APP_PATH=$(find "$MOUNT" -maxdepth 1 -name "*.app" -print 2>/dev/null | head -1)
if [ -z "$APP_PATH" ]; then
  echo "Error: no .app bundle inside the DMG."
  exit 1
fi
APP_NAME=$(basename "$APP_PATH")

echo "Installing $APP_NAME..."
rm -rf "/Applications/$APP_NAME"
cp -R "$APP_PATH" /Applications/

# Strip Gatekeeper's quarantine attribute so the app launches on first run.
# Becomes a no-op once builds are signed + notarized.
xattr -dr com.apple.quarantine "/Applications/$APP_NAME" 2>/dev/null || true

echo "Launching..."
open "/Applications/$APP_NAME"
echo "Done! TTP is ready."
