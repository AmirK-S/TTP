#!/bin/bash
# Load API keys from .dev-keys and run tauri dev
# This avoids keychain password prompts during development

# Load keys
if [ -f .dev-keys ]; then
    export $(grep -v '^#' .dev-keys | xargs)
    echo "✓ Loaded API keys from .dev-keys"
else
    echo "⚠ No .dev-keys file found"
fi

# Run tauri dev
npm run tauri dev
