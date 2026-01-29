# Phase 4: Platform Parity - Research

**Researched:** 2026-01-29
**Domain:** Cross-platform distribution, code signing, notarization, and Windows parity
**Confidence:** MEDIUM (varies by area)

## Summary

This phase addresses three major areas: (1) macOS app distribution with code signing and notarization, (2) Windows installer creation and code signing, and (3) ensuring feature parity across platforms including paste simulation, permissions, and customizable shortcuts.

The research reveals that Tauri 2.x has well-documented workflows for both macOS and Windows distribution. The macOS notarization workflow requires an Apple Developer account ($99/year) and specific environment variables. Windows signing uses standard code signing certificates with PFX files. For paste simulation, the current AppleScript approach on macOS needs a Windows equivalent using enigo (already in dependencies).

**Primary recommendation:** Use Tauri's built-in bundler with GitHub Actions for CI/CD, enigo for cross-platform paste simulation (replacing AppleScript on macOS too for consistency), and implement shortcut customization by dynamically unregistering/registering shortcuts at runtime.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tauri-apps/tauri-action | v0 | GitHub Actions build/release | Official Tauri action for CI/CD |
| enigo | 0.2 | Cross-platform keyboard simulation | Already in project, works on Windows/macOS/Linux |
| tauri-plugin-global-shortcut | 2 | Customizable shortcuts | Already in project, supports register/unregister |
| tauri-plugin-macos-permissions | 0.1+ | macOS permission checks | Official way to check accessibility/mic permissions |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tauri-plugin-store | 2 | Settings persistence | For storing custom shortcut settings |
| WiX Toolset | v3 | MSI installer creation | Windows-only, auto-used by Tauri bundler |
| NSIS | - | Setup.exe installer | Cross-compile capable, Tauri bundler default |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| enigo | AppleScript + PowerShell | Platform-specific code paths, harder to maintain |
| enigo | rdev | rdev is more for listening than simulating, less focused |
| MSI (WiX) | NSIS only | NSIS can cross-compile from macOS, MSI is Windows-only |

**Installation:**
```bash
# Already in project - enigo, global-shortcut
# Add for permissions:
cargo add tauri-plugin-macos-permissions --target 'cfg(target_os = "macos")'
npm add tauri-plugin-macos-permissions-api
```

## Architecture Patterns

### Recommended Project Structure
```
src-tauri/
├── src/
│   ├── paste/
│   │   ├── mod.rs           # Platform abstraction
│   │   ├── clipboard.rs     # Existing clipboard ops
│   │   ├── simulate.rs      # Use enigo for all platforms
│   │   └── permissions.rs   # Platform-conditional permission checks
│   ├── shortcuts.rs         # Add dynamic registration
│   └── platform/            # NEW: Platform-specific code
│       ├── mod.rs
│       ├── macos.rs         # macOS permissions, entitlements
│       └── windows.rs       # Windows-specific behaviors
├── Entitlements.plist       # NEW: macOS entitlements
├── Info.plist               # NEW: macOS permission descriptions
└── tauri.conf.json          # Bundle configuration
```

### Pattern 1: Conditional Compilation for Platform Code
**What:** Use `#[cfg(target_os = "...")]` to isolate platform-specific implementations
**When to use:** Any code that differs between macOS and Windows
**Example:**
```rust
// Source: https://blog.masteringbackend.com/cfg-conditional-compilation-in-rust

#[cfg(target_os = "macos")]
pub fn check_accessibility() -> bool {
    // Use tauri-plugin-macos-permissions
    // ...
}

#[cfg(target_os = "windows")]
pub fn check_accessibility() -> bool {
    // Windows doesn't require explicit accessibility permission
    // for keyboard simulation via SendInput API
    true
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn check_accessibility() -> bool {
    true // Linux, etc.
}
```

### Pattern 2: Unified Paste Simulation with enigo
**What:** Replace AppleScript with enigo for consistent cross-platform paste
**When to use:** For simulating Cmd/Ctrl+V paste keystroke
**Example:**
```rust
// Source: https://docs.rs/enigo/latest/enigo/

use enigo::{Enigo, Key, Keyboard, Settings, Direction::{Press, Click, Release}};

pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create Enigo: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        enigo.key(Key::Meta, Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Meta, Release).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        enigo.key(Key::Control, Press).map_err(|e| e.to_string())?;
        enigo.key(Key::Unicode('v'), Click).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Release).map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

### Pattern 3: Dynamic Shortcut Registration
**What:** Allow users to change shortcuts at runtime
**When to use:** For CFG-04 customizable shortcut requirement
**Example:**
```rust
// Source: https://v2.tauri.app/plugin/global-shortcut/

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

pub fn update_shortcut(
    app: &AppHandle,
    old_shortcut: &str,
    new_shortcut: &str
) -> Result<(), Box<dyn std::error::Error>> {
    let global_shortcut = app.global_shortcut();

    // Parse shortcuts
    let old = old_shortcut.parse::<Shortcut>()?;
    let new = new_shortcut.parse::<Shortcut>()?;

    // Unregister old
    global_shortcut.unregister(old)?;

    // Register new with handler
    global_shortcut.on_shortcut(new, move |_app, _shortcut, event| {
        // Handle shortcut
    })?;

    Ok(())
}
```

### Anti-Patterns to Avoid
- **Platform-specific external processes:** Don't use `osascript` or `PowerShell` when enigo works on both platforms
- **Hardcoded shortcuts:** Always load from settings, allow customization
- **Skipping permission checks:** Always check/request permissions before using accessibility features
- **Building Windows installers on macOS:** MSI requires Windows; use NSIS for cross-compile

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Code signing | Manual codesign scripts | Tauri bundler env vars | Handles keychain, notarization, timestamps |
| Windows installer | Custom InnoSetup | Tauri NSIS/WiX bundler | Proper WebView2 installation, registry |
| Keyboard simulation | Platform shell commands | enigo crate | Cross-platform, handles key states properly |
| macOS permissions | Manual osascript checks | tauri-plugin-macos-permissions | Proper permission request dialogs |
| Settings storage | Manual JSON file handling | tauri-plugin-store | Handles paths, file locking, migrations |

**Key insight:** Tauri's bundler handles 90% of distribution complexity. Don't fight it with custom scripts - configure it properly with environment variables and JSON config.

## Common Pitfalls

### Pitfall 1: Notarization Timeouts in CI
**What goes wrong:** GitHub Actions times out waiting for Apple notarization
**Why it happens:** First-time notarization can take 15-60+ minutes; Apple may need backend setup
**How to avoid:**
- Set longer timeout on build step (60+ minutes first time)
- Contact Apple Developer Support if stuck
- Check notarization status manually: `xcrun notarytool history`
**Warning signs:** Build hangs at "notarizing" step for extended periods

### Pitfall 2: Missing Entitlements for Signed macOS Apps
**What goes wrong:** App works in dev mode but crashes when signed
**Why it happens:** Hardened runtime blocks capabilities without explicit entitlements
**How to avoid:**
- Create `Entitlements.plist` with required entitlements
- Include `com.apple.security.device.audio-input` for microphone
- Test signed builds locally before CI
**Warning signs:** App works unsigned, fails when signed; permission denied errors

### Pitfall 3: Windows SmartScreen Warnings
**What goes wrong:** Users see scary "Windows protected your PC" warning
**Why it happens:** No code signing or OV certificate without reputation
**How to avoid:**
- Use EV certificate for immediate trust (expensive)
- Use OV certificate and build reputation over time
- Provide clear installation instructions for users
**Warning signs:** First-time users abandon installation

### Pitfall 4: enigo Accessibility Permission on macOS
**What goes wrong:** enigo silently fails to send keystrokes
**Why it happens:** App not granted Accessibility permission in System Preferences
**How to avoid:**
- Check permission before attempting paste simulation
- Use `tauri-plugin-macos-permissions` to request access
- Show user-friendly dialog explaining why permission is needed
**Warning signs:** Paste silently does nothing, no error returned

### Pitfall 5: WebView2 Not Installed on Windows
**What goes wrong:** App fails to launch on some Windows systems
**Why it happens:** WebView2 runtime not pre-installed (especially Windows 10)
**How to avoid:**
- Configure installer to embed WebView2 bootstrapper
- Set `webviewInstallMode` to `embedBootstrapper` in tauri.conf.json
**Warning signs:** Blank window on launch, or app won't start at all

### Pitfall 6: Shortcut Conflicts
**What goes wrong:** App's global shortcut doesn't trigger
**Why it happens:** Another app already registered that shortcut
**How to avoid:**
- Use `is_registered()` to check before registering
- Handle registration failure gracefully
- Allow users to customize shortcut if default conflicts
**Warning signs:** Shortcut works on some machines but not others

## Code Examples

Verified patterns from official sources:

### macOS Entitlements.plist
```xml
<!-- Source: https://v2.tauri.app/distribute/macos-application-bundle/ -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Required for microphone access with hardened runtime -->
    <key>com.apple.security.device.audio-input</key>
    <true/>
</dict>
</plist>
```

### macOS Info.plist (Permission Descriptions)
```xml
<!-- Source: https://v2.tauri.app/distribute/macos-application-bundle/ -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>NSMicrophoneUsageDescription</key>
    <string>TTP needs microphone access to record your voice for transcription.</string>
</dict>
</plist>
```

### tauri.conf.json Bundle Configuration
```json
{
  "bundle": {
    "macOS": {
      "entitlements": "./Entitlements.plist",
      "signingIdentity": null,
      "hardenedRuntime": true,
      "minimumSystemVersion": "10.15"
    },
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.comodoca.com",
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      }
    }
  }
}
```

### GitHub Actions Workflow (Key Parts)
```yaml
# Source: https://v2.tauri.app/distribute/pipelines/github/
name: Release
on:
  push:
    branches: [release]

jobs:
  build:
    strategy:
      matrix:
        include:
          - platform: macos-latest
            args: --target aarch64-apple-darwin
          - platform: macos-latest
            args: --target x86_64-apple-darwin
          - platform: windows-latest
            args: ""

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: lts/*

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Import Windows Certificate
        if: matrix.platform == 'windows-latest'
        env:
          WINDOWS_CERTIFICATE: ${{ secrets.WINDOWS_CERTIFICATE }}
          WINDOWS_CERTIFICATE_PASSWORD: ${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}
        run: |
          New-Item -ItemType directory -Path certificate
          Set-Content -Path certificate/tempCert.txt -Value $env:WINDOWS_CERTIFICATE
          certutil -decode certificate/tempCert.txt certificate/certificate.pfx
          Import-PfxCertificate -FilePath certificate/certificate.pfx -CertStoreLocation Cert:\CurrentUser\My -Password (ConvertTo-SecureString -String $env:WINDOWS_CERTIFICATE_PASSWORD -Force -AsPlainText)

      - name: Build Tauri App
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # macOS signing
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        with:
          tagName: v__VERSION__
          releaseName: "TTP v__VERSION__"
          args: ${{ matrix.args }}
```

### Check macOS Permissions
```javascript
// Source: https://github.com/ayangweb/tauri-plugin-macos-permissions
import {
  checkAccessibilityPermission,
  requestAccessibilityPermission,
  checkMicrophonePermission,
  requestMicrophonePermission
} from "tauri-plugin-macos-permissions-api";

async function ensurePermissions() {
  // Check accessibility (for paste simulation)
  const hasAccessibility = await checkAccessibilityPermission();
  if (!hasAccessibility) {
    await requestAccessibilityPermission();
  }

  // Check microphone
  const hasMic = await checkMicrophonePermission();
  if (!hasMic) {
    await requestMicrophonePermission();
  }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| OV certificates easy to get | EV certs needed for instant trust | June 2023 | SmartScreen warnings without EV |
| Tauri 1.x signing | Tauri 2.x bundler with env vars | Oct 2024 | Simpler, more automated |
| Manual notarization | Tauri bundler auto-notarizes | Tauri 2.0 | Just set env vars |
| App Store Connect login | App Store Connect API keys | Apple recommendation | More secure for CI/CD |

**Deprecated/outdated:**
- `codesign` manual commands: Use Tauri bundler's automatic signing
- OV certificates for instant trust: Since June 2023, only EV gets immediate SmartScreen trust

## Open Questions

Things that couldn't be fully resolved:

1. **fn/Globe Key Capture on macOS**
   - What we know: fn key is handled at hardware level, CGEventTap may not capture it reliably
   - What's unclear: Whether any public API can detect fn key presses consistently
   - Recommendation: Keep this as "Future Enhancement" - standard modifier keys (Option, Ctrl, etc.) work reliably. Users can remap fn to Caps Lock in System Preferences if needed.

2. **EV vs OV Certificate Cost/Benefit**
   - What we know: EV eliminates SmartScreen warnings immediately, OV builds reputation over time
   - What's unclear: Exact cost, renewal process, whether free alternatives exist
   - Recommendation: Start with OV certificate (~$100-200/year), upgrade to EV if user complaints warrant it

3. **enigo vs AppleScript Reliability**
   - What we know: AppleScript works currently, enigo is more cross-platform
   - What's unclear: Whether enigo has same edge cases that caused original switch to AppleScript
   - Recommendation: Test enigo thoroughly on macOS before replacing AppleScript. If issues arise, keep platform-specific paths.

## Sources

### Primary (HIGH confidence)
- [Tauri v2 macOS Code Signing](https://v2.tauri.app/distribute/sign/macos/) - Signing and notarization workflow
- [Tauri v2 Windows Installer](https://v2.tauri.app/distribute/windows-installer/) - NSIS and MSI options
- [Tauri v2 Windows Code Signing](https://v2.tauri.app/distribute/sign/windows/) - Certificate configuration
- [Tauri v2 Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/) - Registration API
- [Tauri v2 macOS Application Bundle](https://v2.tauri.app/distribute/macos-application-bundle/) - Entitlements and Info.plist
- [Tauri v2 GitHub Actions](https://v2.tauri.app/distribute/pipelines/github/) - CI/CD workflow

### Secondary (MEDIUM confidence)
- [enigo crate docs](https://docs.rs/enigo/latest/enigo/) - Keyboard simulation API
- [tauri-plugin-macos-permissions](https://github.com/ayangweb/tauri-plugin-macos-permissions) - Permission checking
- [tauri-plugin-mic-recorder](https://github.com/ayangweb/tauri-plugin-mic-recorder) - Windows audio support confirmed
- [rdev crate](https://github.com/Narsil/rdev) - Alternative keyboard library (reference)

### Tertiary (LOW confidence)
- [fn key limitations](https://github.com/zmkfirmware/zmk/issues/947) - Hardware-level fn key handling
- Community discussions on notarization timeouts

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Official Tauri documentation
- macOS signing/notarization: HIGH - Well-documented official workflow
- Windows signing: MEDIUM - Documented, but EV cert details unclear
- Cross-platform paste: MEDIUM - enigo documented, but switching from AppleScript untested
- Shortcut customization: HIGH - Official plugin API
- fn key capture: LOW - Fundamental hardware limitation, likely not feasible

**Research date:** 2026-01-29
**Valid until:** 2026-03-29 (60 days - Tauri 2.x is stable, signing processes don't change often)
