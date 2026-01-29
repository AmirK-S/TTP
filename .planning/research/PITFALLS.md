# Pitfalls Research

**Domain:** Cross-platform voice transcription desktop app (Tauri)
**Researched:** 2026-01-29
**Confidence:** HIGH (verified via official Tauri docs, GitHub issues, community reports)

---

## Critical Pitfalls

### Pitfall 1: macOS Microphone Permission Not Prompting After Code Signing

**What goes wrong:**
The app works perfectly in development mode, but after signing and notarization, the microphone permission dialog never appears. Audio recording silently fails with no user-facing error.

**Why it happens:**
macOS requires `NSMicrophoneUsageDescription` in Info.plist AND proper entitlements. When the app is signed, macOS enforces stricter permission checks. Many developers only test in dev mode where these checks are relaxed.

**How to avoid:**
1. Create `Info.plist` in `src-tauri/` with `NSMicrophoneUsageDescription` key
2. Create `entitlements.plist` with appropriate App Sandbox entitlements
3. Reference entitlements in `tauri.conf.json` under `bundle.macOS.entitlements`
4. Test signed builds locally BEFORE submitting for notarization
5. Consider using `tauri-plugin-macos-permissions` for programmatic permission checking

**Warning signs:**
- Works in `tauri dev` but not in `tauri build`
- Works unsigned but not signed
- No permission dialog appears on first launch
- `navigator.mediaDevices.getUserMedia` returns `NotAllowedError`

**Phase to address:**
Phase 1 (Core Foundation) - Must be configured from the start

---

### Pitfall 2: WebView Microphone Permissions Not Persisting

**What goes wrong:**
Users are prompted for microphone permission every time the app restarts, even after granting permission. Two separate permission dialogs appear (app-level and WebView-level).

**Why it happens:**
Tauri's WebView on macOS has a known bug where webview-level permissions are not persisted across app restarts. The application-level permission (via Info.plist) persists, but the WebView's internal permission state resets.

**How to avoid:**
1. Capture audio from Rust side using `cpal` crate instead of WebView's `getUserMedia`
2. If using WebView audio, implement a permission status check on app launch
3. Display clear UI explaining why permission is needed again
4. Consider caching permission state in app settings and showing appropriate messaging

**Warning signs:**
- Users complain about repeated permission prompts
- Permission dialogs appear differently in dev vs production
- Two permission dialogs (system and webview) appearing

**Phase to address:**
Phase 2 (Audio Capture) - Architectural decision: Rust-side vs WebView-side audio

---

### Pitfall 3: Global Hotkey Conflicts with Other Applications

**What goes wrong:**
Your app registers a global hotkey that conflicts with another popular app (like Figma's Cmd+Opt+G). When your app is running, it intercepts the shortcut, breaking other apps. Users blame your app and uninstall it.

**Why it happens:**
Global hotkeys are registered at the system level. The most recent app to register "wins." Popular shortcut combinations are likely already in use by other apps, and users have muscle memory for them.

**How to avoid:**
1. Use uncommon modifier combinations (e.g., Cmd+Shift+Opt rather than just Cmd+Opt)
2. Make hotkeys user-configurable from day one
3. Check if shortcut is already registered before claiming it
4. Provide clear UI to resolve conflicts
5. Document which shortcuts are used and common conflicts

**Warning signs:**
- User reports that "X app stopped working when Y is running"
- Hotkey works inconsistently
- `global-shortcut:allow-register` fails silently

**Phase to address:**
Phase 3 (Hotkey Integration) - User-configurable hotkeys from the start

---

### Pitfall 4: Accessibility Permission Corruption on macOS

**What goes wrong:**
Auto-paste functionality (simulating Cmd+V) stops working randomly, even though Accessibility permissions appear to be granted in System Preferences. Users toggle permissions on/off but nothing fixes it.

**Why it happens:**
macOS maintains an internal database of accessibility permissions that can become corrupted, especially after OS updates. The checkboxes in System Preferences may not reflect the actual permission state.

**How to avoid:**
1. Implement robust permission checking at app startup
2. Guide users through the "remove and re-add" process when detection fails
3. Store a "last known good" permission state and compare
4. Provide a "Repair Permissions" button in settings that automates the fix
5. Detect macOS version changes and proactively warn users

**Warning signs:**
- "Not allowed to send keystrokes" errors despite permissions appearing granted
- Auto-paste worked before but stopped after OS update
- Users report the app is checked in Accessibility but still doesn't work

**Phase to address:**
Phase 4 (Auto-Paste) - Include permission repair flow in initial implementation

---

### Pitfall 5: Cross-Platform Build Fails from Single Machine

**What goes wrong:**
Developer tries to build Windows installer from macOS or vice versa. Build fails with cryptic errors about missing tools, or builds succeed but produce broken binaries.

**Why it happens:**
Tauri relies on native toolchains that don't support cross-compilation well. MSI installers require Windows-only WiX toolkit. NSIS cross-compilation is possible but fragile and undertested.

**How to avoid:**
1. Use GitHub Actions from day one with Tauri's official CI workflow
2. Never promise features based on untested cross-compiled builds
3. Test on real hardware/VMs for each platform before release
4. Set up platform-specific CI jobs that run on native runners

**Warning signs:**
- "WiX is not available" errors on non-Windows machines
- `clang-cl: error: unsupported option` when cross-compiling
- App installs but crashes immediately on target platform

**Phase to address:**
Phase 1 (Core Foundation) - CI/CD setup must happen immediately

---

### Pitfall 6: Notarization Stuck or Failing

**What goes wrong:**
The build process hangs indefinitely at "notarizing" step, sometimes for hours. Or notarization completes but Apple rejects the submission without clear error messages.

**Why it happens:**
Apple's notarization service can be slow or have outages. External binaries (sidecars) are not automatically signed, causing rejection. Entitlements may be incorrect or missing. Free Apple Developer accounts cannot notarize at all.

**How to avoid:**
1. Use a paid Apple Developer account ($99/year) for distribution
2. Set `APPLE_TEAM_ID` environment variable
3. Implement timeout handling and retry logic in CI
4. Use `xcrun notarytool history` to check submission status
5. Review notarization logs when submissions fail
6. Sign all sidecars/external binaries explicitly

**Warning signs:**
- Build takes >30 minutes on the notarization step
- `xcrun notarytool submit` returns timeout errors
- Submission status shows "In Progress" for hours

**Phase to address:**
Phase 5 (Distribution) - But test notarization early with minimal builds

---

### Pitfall 7: Different Paste Behavior Across Applications

**What goes wrong:**
Auto-paste works in most apps but fails silently in specific applications (Microsoft Office, some password managers, terminal emulators). Text ends up in wrong field or doesn't appear at all.

**Why it happens:**
Applications handle keyboard input differently. Some apps (like Microsoft Office) don't use standard macOS keyboard shortcut handling. Some apps block paste programmatically for security. Some apps have focus timing issues.

**How to avoid:**
1. Add configurable delay before paste (default 50-100ms)
2. Offer "Type Text" mode as fallback (simulate individual keystrokes)
3. Build an app compatibility list and adjust behavior per-app
4. Let users configure per-application paste behavior
5. Detect the frontmost app and apply known workarounds

**Warning signs:**
- "Works in Notes but not in Word"
- Paste appears delayed or partial
- Focus jumps to wrong window/field

**Phase to address:**
Phase 4 (Auto-Paste) - Include type-simulation fallback from the start

---

### Pitfall 8: Tray/Menu Bar Icon Disappears or Crashes App

**What goes wrong:**
On macOS, the tray icon randomly disappears. Or when all windows are closed and user clicks menu bar, the app crashes. On Windows, closing the main window kills the entire app instead of minimizing to tray.

**Why it happens:**
Tauri has known bugs with tray icons on macOS (high-priority bug as of 2025). Menu event handlers crash when no windows exist. Windows and macOS have fundamentally different "close" semantics.

**How to avoid:**
1. Keep an invisible window alive on macOS to prevent menu crashes
2. Intercept `CloseRequested` events and call `api.prevent_close()` + `window.hide()`
3. Test tray behavior extensively on both platforms
4. Pin to specific Tauri versions known to work; don't auto-update
5. Implement fallback UI if tray icon fails to appear

**Warning signs:**
- App works as window-based app but crashes as tray app
- Tray icon appears on Windows but not macOS (or vice versa)
- Menu clicks crash the app when no windows are open

**Phase to address:**
Phase 1 (Core Foundation) - Tray architecture must be correct from start

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcoded hotkeys | Ship faster | Users can't customize, conflicts inevitable | Never for global hotkeys |
| WebView audio instead of Rust audio | Faster to prototype | Permission persistence bugs, Linux won't work | Prototype only |
| Skip notarization testing | Faster dev cycle | "Works on my machine" but fails for users | Never for macOS distribution |
| Single-platform dev (macOS only) | Faster iteration | Windows bugs pile up, discovered at release | Only for first 2 weeks |
| No permission status checking | Simpler code | Users confused when things silently fail | Never |
| Fixed paste delay | Simpler implementation | Breaks in slow/fast apps | MVP only, make configurable quickly |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| OpenAI Whisper API | Expecting streaming support | whisper-1 does not support streaming; chunk audio and send sequentially |
| OpenAI Whisper API | Sending huge audio files | Chunk at sentence boundaries or 30-second intervals for faster feedback |
| macOS Accessibility | Assuming permission = capability | Check permission AND test actual keystroke sending |
| Windows audio devices | Using default device without selection | Enumerate devices and let user choose; defaults change |
| GitHub Actions | Building only on push | Also build on schedule to catch upstream Tauri breakages |
| Tauri plugins | Using latest versions | Pin versions; plugin updates can break builds |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Recording to memory | App works fine | Stream to temp file; limit buffer size | >60 second recordings, low-RAM devices |
| Synchronous API calls | UI freezes during transcription | Run Whisper API call in background thread/task | Any network latency |
| No audio level monitoring | Users don't know if recording | Show real-time audio level indicator | Users think app is broken |
| Polling for hotkey state | High CPU usage | Use event-driven hotkey registration | Always problematic on laptops |
| Large audio file uploads | Slow, timeout errors | Compress to webm/opus before upload | Recordings >30 seconds |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Storing OpenAI API key in plain text config | Key theft if app data is compromised | Use OS keychain (macOS Keychain, Windows Credential Manager) |
| Logging audio file paths | Privacy leak in logs | Never log full paths; use hashes or temp IDs |
| Not clearing audio after transcription | Audio files accumulate, privacy risk | Delete temp audio immediately after successful transcription |
| Transmitting audio over HTTP | Man-in-the-middle interception | Always use HTTPS; OpenAI API requires it anyway |
| Clipboard contains previous transcription | Unexpected paste of sensitive content | Clear clipboard after paste or use direct text injection |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No visual recording indicator | Users don't know if recording is active | Tray icon changes color/animation when recording |
| Silent failures | Users think app is broken | Always show status: "Connecting...", "Transcribing...", "Done" |
| Auto-paste without confirmation | Text goes to wrong app | Brief toast showing transcribed text before/during paste |
| Long transcription delays without feedback | Users re-trigger recording | Show progress indicator; allow cancellation |
| Hotkey conflicts without notification | Other apps break mysteriously | Detect and warn about conflicts on startup |
| No way to edit before paste | Typos from Whisper get pasted | Offer quick-edit window before auto-paste (configurable) |

---

## "Looks Done But Isn't" Checklist

- [ ] **Microphone permission:** Tested on SIGNED build, not just dev mode
- [ ] **Global hotkey:** Tested with other apps running that use similar shortcuts
- [ ] **Auto-paste:** Tested in at least 5 different apps (Notes, Word, Terminal, browser, VS Code)
- [ ] **Tray icon:** Tested after closing all windows (macOS crash test)
- [ ] **Windows build:** Actually tested on Windows, not just cross-compiled
- [ ] **Notarization:** Complete notarization cycle tested, not just local signing
- [ ] **Permission dialogs:** Tested "deny" flow and recovery, not just "allow"
- [ ] **Audio recording:** Tested with external microphones, not just built-in
- [ ] **Long recordings:** Tested 2+ minute recordings, not just quick phrases
- [ ] **Network errors:** Tested with no internet, slow internet, and API rate limits

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Permission database corruption | LOW | Guide user to remove/re-add app in System Preferences |
| Notarization rejection | MEDIUM | Review logs, fix entitlements, re-sign, re-submit |
| Hotkey conflicts | LOW | Provide settings UI to change hotkeys |
| Tray icon disappeared | MEDIUM | Restart app; if persistent, check Tauri version, may need upgrade |
| Audio not recording | LOW | Check device selection; guide user to System Preferences |
| Cross-platform build failures | HIGH | Set up proper CI/CD; never attempt cross-compilation |
| WebView permission bugs | HIGH | Migrate to Rust-side audio capture (architectural change) |
| Paste failing in specific apps | LOW | Enable "type text" fallback mode |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Microphone permission not prompting | Phase 1 (Foundation) | Test signed build requests permission on fresh macOS install |
| WebView permission persistence | Phase 2 (Audio) | Test permission survives app restart |
| Global hotkey conflicts | Phase 3 (Hotkeys) | Test with Figma, VS Code, browser dev tools open |
| Accessibility permission corruption | Phase 4 (Paste) | Test after simulated OS update (remove/re-add app) |
| Cross-platform build failures | Phase 1 (Foundation) | CI produces working installers for both platforms |
| Notarization failures | Phase 5 (Distribution) | Full notarization completes in CI |
| Different paste behavior | Phase 4 (Paste) | Paste works in Word, Terminal, Notes, browser |
| Tray icon issues | Phase 1 (Foundation) | Tray icon visible and clickable after all windows closed |

---

## Sources

**Official Documentation:**
- [Tauri macOS Application Bundle](https://v2.tauri.app/distribute/macos-application-bundle/)
- [Tauri macOS Code Signing](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [Tauri Permissions](https://v2.tauri.app/security/permissions/)

**GitHub Issues (verified bugs/problems):**
- [Microphone permission not prompted on signed builds](https://github.com/tauri-apps/tauri/issues/9928)
- [WebView permissions not remembered](https://github.com/tauri-apps/tauri/issues/8979)
- [macOS tray icon not appearing](https://github.com/tauri-apps/tauri/issues/13770)
- [macOS crash when using menubar without windows](https://github.com/tauri-apps/tauri/issues/8812)
- [Global shortcut setup panic](https://github.com/tauri-apps/plugins-workspace/issues/2540)
- [Notarization issues with sidecars](https://github.com/tauri-apps/tauri/issues/11992)
- [getUserMedia permission errors](https://github.com/tauri-apps/tauri/issues/8314)

**Community Resources:**
- [tauri-plugin-macos-permissions](https://github.com/ayangweb/tauri-plugin-macos-permissions)
- [tauri-plugin-mic-recorder](https://github.com/ayangweb/tauri-plugin-mic-recorder)
- [macOS Accessibility permission fixes](https://www.macworld.com/article/347452/how-to-fix-macos-accessibility-permission-when-an-app-cant-be-enabled.html)
- [Keyboard Maestro Accessibility Permission Problem](https://wiki.keyboardmaestro.com/assistance/Accessibility_Permission_Problem)
- [OpenAI Whisper API Reference](https://platform.openai.com/docs/api-reference/audio/)

---
*Pitfalls research for: TTP (Talk To Paste) - Cross-platform voice transcription*
*Researched: 2026-01-29*
