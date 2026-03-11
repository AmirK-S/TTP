#!/bin/sh
# TTP - Talk To Paste installer
# Usage: curl -sL ttp.amirks.eu/install.sh | sh
set -e

DMG="/tmp/TTP.dmg"
VOL="/Volumes/TTP by AmirKS"
APP="TTP by AmirKS.app"
URL="https://github.com/AmirK-S/TTP/releases/latest/download/TTP-macOS-arm64.dmg"

echo "Downloading TTP..."
curl -sL "$URL" -o "$DMG"
echo "Installing..."
hdiutil attach "$DMG" -quiet
cp -R "$VOL/$APP" /Applications/
hdiutil detach "$VOL" -quiet
rm "$DMG"
echo "Launching TTP..."
open "/Applications/$APP"
echo "Done! TTP is ready."
