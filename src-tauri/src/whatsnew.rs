// TTP - Talk To Paste
// "What's New" version tracking. Shows changelog after app update.

use std::fs;
use std::path::PathBuf;
use tauri::command;

/// Get the config directory (same as permissions.rs)
fn get_config_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ttp");

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }

    config_dir
}

/// Path to the file that stores the last version the user has seen
fn last_seen_version_path() -> PathBuf {
    get_config_dir().join("last_seen_version")
}

/// Current app version (from Cargo.toml / tauri.conf.json)
fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Hardcoded changelogs per version.
/// Returns None if no changelog is available for that version.
fn changelog_for(version: &str) -> Option<&'static str> {
    match version {
        "2.1.9" => Some(
            "Auto-update will no longer yank a recording out from under you mid-session.\n\
             • The 60-second idle timer from v2.1.6 wasn't enough: a user who opened TTP, did something else for ~60s, then pressed Fn would land on the restart firing right as they began recording. Same end result as the original bug, just delayed.\n\
             • New rule: the moment you start a recording in this session, auto-restart is off until your next quit + relaunch. You'll pick up the new bundle on your next natural app open — the tray \"Install update (vX.Y.Z)\" menu item is still there if you want to relaunch sooner.\n\
             • The 60-second safety net is kept only for the genuinely-idle case (TTP opened, never used, walked away).",
        ),
        "2.1.8" => Some(
            "Onboarding cleanup: Settings no longer pops up behind the wizard, and your preference toggles actually stick.\n\
             • Fixed: Settings was auto-opening behind onboarding to surface the WhatsNew changelog, then loading its toggle state from disk before the wizard had saved the preferences. Any later interaction with Settings would silently overwrite the wizard's choices. Now WhatsNew is suppressed during onboarding, and Settings opens only after you click Finish, with the right toggles already checked.\n\
             • Cross-window settings sync hardened: every open window now mirrors saved changes into its own state immediately, so a setting changed in one place can't be reverted by a stale snapshot in another.\n\
             • Auto-update path on the GitHub Releases CDN was caching v2.1.5 for hours after newer builds shipped. The workflow now flips drafts to published on its own so the manifest stays current.",
        ),
        "2.1.7" => Some(
            "Stops the AI Polish upgrade nudge from spamming once a month.\n\
             • Free users who hit the 30/month AI Polish cap on Windows were getting a fresh \"upgrade to TTP Pro\" toast on every recording past the cap (macOS grouped them, Windows didn't). It read as bloatware — fair complaint.\n\
             • TTP now surfaces the notification at most once per calendar month. The cap is still enforced and your usage is still tracked exactly as before. Just no more per-recording toasts.\n\
             • Rolls over naturally: if you cross into next month and hit the cap again, you'll see the message one more time.",
        ),
        "2.1.6" => Some(
            "Fixes the empty TTP window that appeared after updating, plus a kinder update timing.\n\
             • Empty \"TTP\" window after an update is gone. The window-state plugin was over-eagerly tracking every window on disk and could re-show the hidden shell that hosts background checks. Now scoped to just Settings, position + size only.\n\
             • Stale window state from previous versions is ignored automatically (no action needed on your end). Settings will open at its default size once after this update, then remember your preferred size from now on.\n\
             • Auto-restart after a silent update now waits 60 seconds of idle before relaunching, so it can no longer yank the app out from under you while you're about to press Fn.\n\
             • Defensive belt-and-suspenders: the hidden background window is now force-hidden during startup, so no future regression can surface it as an empty \"TTP\" window.",
        ),
        "2.1.5" => Some(
            "Light / Dark / System appearance switcher + sticky Settings sidebar.\n\
             • New Settings → General → Appearance section: pick System (follows macOS, default), Light, or Dark and the choice sticks across launches.\n\
             • Inline bootstrap script reads the saved choice before any CSS paints, so opening a window in forced-dark on a light macOS no longer flashes white.\n\
             • Choice syncs across every TTP window in real time. Change it in Settings and the pill, onboarding, and tray all flip together.\n\
             • Settings sidebar (logo, version, nav) is now truly fixed — it no longer drifts upward when you scroll deep sections like Pro or Advanced. The right pane scrolls on its own.",
        ),
        "2.1.4" => Some(
            "One-click uninstall.\n\
             • Settings → Advanced → \"Uninstall TTP completely\" now actually removes everything: the .app from /Applications, your settings, history, dictionary, license, all three Keychain entries, the LaunchAgent if autostart was on, WebKit caches, and the macOS TCC permissions. The previous answer was \"drag to trash and hope\" which left half a dozen things behind.\n\
             • macOS-only for now. Windows / Linux still need manual removal (the README has the paths).",
        ),
        "2.1.3" => Some(
            "Onboarding fixes + Settings sidebar gets your stats.\n\
             • Preferences toggles in the wizard actually persist now. Two of three (hide pill, crash reports) were silently bouncing back because they bypassed the settings store and called the backend with a partial Settings object the Rust side couldn't deserialize. Routed through the same store the Settings panel uses.\n\
             • Tour step 3 illustration is now a mini macOS menu bar mockup with the TTP icon highlighted, instead of a microphone glyph that read as the keyboard mic key.\n\
             • Your stats moved to the bottom-left sidebar in Settings (sleek one-liner: words this month + transcription count). No more scrolling past the Pro section to find your numbers.\n\
             • Sidebar LinkedIn link relabelled \"Follow on LinkedIn\" with an external-link glyph so it's obviously an outbound action.",
        ),
        "2.1.2" => Some(
            "Onboarding gets a Preferences step and a Tour.\n\
             • New \"A few quick choices\" step before the finish line: three opt-in toggles for hiding the pill when inactive, launching TTP at startup, and sharing anonymous crash reports. All default OFF (privacy first); change later from Settings.\n\
             • New \"Here's how it works\" tour at the end: three illustrated cards walking you through hold-to-talk, the pill states, and where to find TTP later (menu bar + Spotlight). Closes the \"I finished the wizard, now what?\" gap.\n\
             • Onboarding wizard is now five steps (Welcome → Permissions → Groq → Preferences → Tour) instead of three. Step dots updated to match.",
        ),
        "2.1.1" => Some(
            "Settings rebuilt. New icon. Pill goes smooth.\n\
             • Settings is now five clear sections (General, Capture, Pro, Data, Advanced) instead of eleven stacked panels. Launch-at-startup moved out of Recording Mode where it never belonged.\n\
             • All Settings buttons, inputs, modals, and section cards now share the same primitives as the rest of the app. No more two-tone shadcn cousin pasted next to the premium pill.\n\
             • Empty states for History, Dictionary, and Usage actually look designed instead of \"Empty.\"\n\
             • Onboarding tells the truth about your voice (it goes to Groq, not local), shows live trial countdown to the minute, and explains where TTP lives (menu bar / Spotlight) before you close the window.\n\
             • The Pro upgrade link now points to the real site (ttp.amirks.eu) and every paywall CTA quotes the actual €17.\n\
             • New light app icon. The dark one is still bundled as backup. Light reads better in Spotlight and Raycast.\n\
             • Pill waveform is now GPU-composited (transform: scaleY, no layout thrash). Smoother on Intel and battery-saver mode.\n\
             • Lazy-loaded windows: the pill no longer ships Settings.tsx's 38 KB it never renders. Sentry only loads if you consented to crash reports.\n\
             • Pre-warmed Groq TLS handshake at startup. First dictation feels instant instead of paying ~300 ms of network setup.\n\
             • prefers-reduced-motion respected globally. Vestibular-safe.\n\
             • Settings window position and size now persist across launches.\n\
             • Em-dashes purged from 106 places across the UI. Section titles in sentence case. Errors tell you what to do, not just what broke.\n\
             • Landing site (ttp.amirks.eu) rebuilt on the same brand-blue light theme with dark-mode fallback.",
        ),
        "2.0.9" => Some(
            "Hands-free mode behaves like you'd expect\n\
             • macOS: double-tap Fn now actually enters hands-free mode. The previous detection threshold required each tap to last at least 150 ms; natural double-taps are ~50–100 ms, so the gesture silently never registered. Lowered the candidate threshold to 20 ms.\n\
             • Windows / cross-platform: double-tap is now a TRANSIENT override for one recording, not a permanent settings change. Before, a single double-tap quietly flipped the persisted preference to hands-free, so every subsequent single press also entered hands-free until you dug into Settings to turn it back off, while the Settings panel still showed it as disabled. State and behavior now match.\n\
             • The Settings → \"Hands-free mode\" toggle is unchanged: that's still the persistent preference (single press to start/stop). Double-tap just lets you opt into hands-free for one recording without touching it.",
        ),
        "2.0.8" => Some(
            "Big numbers stay legible\n\
             • Usage stats now use compact notation (1.2K, 234K, 1.2M) so a power user with millions of words doesn't overflow the column. Locale-aware: French shows \"1,2 M\", English shows \"1.2M\".",
        ),
        "2.0.7" => Some(
            "Compact stats row\n\
             • Your usage section is now a single tight row of three columns (Week / Month / All time). Takes ~1/3 the vertical space of the previous card layout while keeping the same info.",
        ),
        "2.0.6" => Some(
            "Usage stats: now at the top + live refresh\n\
             • Your usage section is now the very first thing in Settings. The about block moved below it.\n\
             • Stats update automatically when a transcription finishes, no need to close and reopen Settings to see the new count.",
        ),
        "2.0.5" => Some(
            "Your usage, visible at last\n\
             • New Settings → \"Your usage\" section shows how much you've transcribed: words, characters, and transcriptions for this week, this month, and all time.\n\
             • 100% local. Stats are computed from your existing transcription history, nothing leaves your machine.\n\
             • Starts counting from this update; older transcriptions aren't backfilled.",
        ),
        "2.0.4" => Some(
            "Crash class eliminated\n\
             • Removed the third-party analytics plugin that was the root cause of a recurring \"no reactor running\" panic seen since v2.0.2-beta.5 (Sentry TTP-A/B/D/E). Symbolicated stack finally fingered it: the plugin's background flush task called reqwest from outside a Tokio runtime context.\n\
             • Crash reporting via Sentry is unaffected. Only product analytics were removed (nobody was looking at them anyway).\n\
             • One fewer dependency, one fewer HTTP endpoint, one fewer surface for future bugs.",
        ),
        "2.0.3" => Some(
            "Silent auto-updates\n\
             • TTP now downloads and installs updates in the background. No more buried prompts you have to dig through Settings to find.\n\
             • When an update is ready, the menu-bar icon gets a small blue dot and the tray menu shows \"Install update (vX.Y.Z)\". One click and TTP relaunches into the new version.\n\
             • Even if you ignore the dot, the new version takes effect automatically the next time you quit and reopen TTP. The .app on disk has already been updated in place.\n\
             • Same idle-aware gating as before: updates never install while you're recording.",
        ),
        "2.0.2" => Some(
            "TTP 2.0.2\n\
             • TTP now speaks French. Auto-detects your system language, switch any time from Settings → Language. UI, tray menu, and OS notifications all flip together.\n\
             • Self-update finally relaunches the app cleanly on macOS (was silently failing on betas).\n\
             • Gatekeeper no longer re-prompts after an update. The quarantine flag is stripped post-install.\n\
             • Tray icon shows a red dot when Input Monitoring permission is missing. One-click deep link to System Settings.\n\
             • macOS function keys (F3 / F4 / F6 / Mission Control / Launchpad) can no longer trigger recordings even when held.\n\
             • Symbolicated crash reports. Future panics resolve to real function names instead of raw addresses.\n\
             • Internal: three remaining Tokio runtime call sites hardened to prevent the \"no reactor running\" panic some users hit on macOS 26.",
        ),
        "2.0.2-beta.9" => Some(
            "Defensive cleanup + symbolicated crash reports\n\
             • Three remaining raw `tokio::task::spawn_blocking` / `std::thread::spawn` call sites converted to `tauri::async_runtime::spawn_blocking` (paste pipeline, dictionary detection poll, post-update xattr cleanup). May or may not address the recurring `no reactor running` panic. Telemetry will tell.\n\
             • CI now uploads debug symbols (.dSYM/.pdb) to Sentry on every release. Future fatal stacks will resolve to real Rust function names instead of `__mh_execute_header`.",
        ),
        "2.0.2-beta.8" => Some(
            "Validation build for the restart-after-update fix\n\
             • No code change. Pure version bump used to verify the new LaunchServices-based restart path in beta.7 works end-to-end.\n\
             • If the \"Restart Now\" button on this update successfully relaunches the app, the fix is confirmed.",
        ),
        "2.0.2-beta.7" => Some(
            "Restart-after-update fixed (for real this time)\n\
             • \"Restart Now\" after an update now uses macOS LaunchServices (`open -n -a`) instead of the standard plugin-process relaunch. The old path was silently failing for beta builds: the current process died but the new one never appeared.\n\
             • Added the missing window:set_title permission so dynamic window titles stop throwing an ACL error on every window mount (Sentry TTP-C).\n\
             • No new features. Pure post-launch cleanup driven by Sentry telemetry.",
        ),
        "2.0.2-beta.6" => Some(
            "TTP now speaks French\n\
             • Auto-detects your system language at first launch (français on a French macOS, English otherwise)\n\
             • Switch any time from Settings → Language. Every window, the tray menu, and OS notifications all flip together\n\
             • Tutoiement throughout the French UI, with the same glossary as the landing page\n\
             • Errors emitted by the Rust backend now flow as translation codes so the pill and notifications speak your language too\n\
             • Internal: parity check in CI guards against missing translations; <html lang> set for screen-readers\n\
             • CI: Windows builds no longer blocked by the MSI bundler on beta versions (NSIS .exe still ships)",
        ),
        "1.8.1" => Some(
            "Launch polish\n\
             • Onboarding now waits for your Groq API key before closing. No more landing in the app with nothing configured.\n\
             • Input Monitoring permission (used for the Fn key) is now requested in onboarding instead of as a surprise system prompt afterwards.\n\
             • Clearer Groq sign-up steps and a reassurance that the free tier covers tens of thousands of transcriptions a month.\n\
             • Webview locked down with a strict Content Security Policy. XSS in the frontend can no longer reach external networks.\n\
             • Future panic reports will include real symbol names (we now upload debug info to Sentry on every release).",
        ),
        "1.8.0" => Some(
            "Security release\n\
             • Your Groq API key is now stored in the macOS Keychain (Windows Credential Manager) instead of a plaintext file. Existing keys migrate automatically on first launch.\n\
             • License and trial state are now signed with a per-machine secret stored in the keychain. A forged file from one Mac can no longer be replayed on another.\n\
             • Constant-time signature comparison closes a small timing side-channel.\n\
             • Email addresses in error reports are now redacted before they leave your machine.\n\
             • Fixed a rare panic on transcription completion (\"no reactor running\") seen by a single user on macOS 26.3.1.\n\
             • Quality-of-life: more Sentry detail on transcription failures, faster Settings open, smaller settings.json recovery on corruption.",
        ),
        "1.7.3" => Some(
            "• Fix: a slow IPC bridge boot could leave the app in a state where every backend call failed for the rest of the session. Now it retries instead of caching the failure.\n\
             • Bumped the transcription rate limit from 5 to 20 per minute so power users don't hit it on a normal workflow (still caps a runaway loop)\n\
             • Settings file now keeps a .bak so a half-written settings.json can recover instead of resetting your preferences\n\
             • Internal: smaller polish from production telemetry feedback",
        ),
        "1.7.2" => Some(
            "• Faster Settings: history and dictionary now cache in memory (5s TTL) instead of re-reading the file on every render. Opening Settings feels instant, even with hundreds of entries.\n\
             • Smoother dictionary list: rows skip re-render when their entry hasn't changed (real win once you've got a few dozen)\n\
             • Better error triage on our side: transcription failures now report the HTTP status code separately so we can spot a Groq outage faster\n\
             • Internal: hardened the release pipeline (auto-retry on Apple notarization flakes, mandatory signing for releases, pinned action versions)",
        ),
        "1.7.1" => Some(
            "• Fixed F3 / F4 / F6 still triggering recording on macOS. The keys macOS hijacks for Mission Control, Launchpad, and Do Not Disturb were slipping past the previous filter.\n\
             • Fixed the \"What's New\" popup growing too tall to show its dismiss button when the changelog ran long\n\
             • Anti-abuse: capped transcription requests at 5 per minute so a runaway loop can't burn through your Groq credits\n\
             • Hardened cold-start: the IPC bridge is now waited on before any backend call, eliminating a class of crashes that hit some users on launch\n\
             • Smaller initial JS payload thanks to code-splitting (Tauri + icon library load on demand)\n\
             • Internal: cleaner timer lifecycle in the transcription and updater hooks; small dead-code cleanup",
        ),
        "1.7.0" => Some(
            "• Onboarding refresh: clearer help text, ordered steps, trial banner, and one-tap link to System Settings when a permission is denied\n\
             • Friendlier error messages when Groq is rate-limited (429) or your API key is invalid (401). No more raw HTTP codes in the pill.\n\
             • You now get a quick \"Polish unavailable, pasting raw text\" notice instead of silent fallback\n\
             • Microphone permission denied mid-session now shows feedback in the pill instead of failing silently\n\
             • Update failures (check + download) are tracked so we can spot regressions; the \"Check for Updates\" button no longer gets stuck after a transient network blip\n\
             • macOS: the quarantine flag is cleared after a self-update so Gatekeeper stops re-prompting you\n\
             • Anti-tamper: license and usage caches are now HMAC-signed; manual file edits no longer hand out free Pro\n\
             • Fewer ways the app can crash on cold start (license cache poison recovery, recordings dir errors)\n\
             • Crash reporter can no longer take itself down on its own regex (defensive)\n\
             • Privacy: audio-backup failures send only a category. No more raw OS paths.\n\
             • Removed listener race condition that could surface as a UI freeze (TTP-5)\n\
             • Background analytics errors fixed (TTP-6/8)",
        ),
        "1.6.2" => Some(
            "• Fixed background analytics errors that were spamming our crash reporter\n\
             • Hardened event listeners against a rare race that could surface as a UI freeze\n\
             • Crash reporter pipeline can no longer panic on its own regex (defensive)\n\
             • Privacy: audio-backup failures send a category instead of the raw OS path\n\
             • No new features. Pure stability pass driven by production telemetry.",
        ),
        "1.6.1" => Some(
            "• Launch at startup: toggle in Settings to open TTP automatically when you log in\n\
             • Fixed F3 / F6 / other system function keys triggering recordings on macOS\n\
             • Pill visibility setting is now respected after restart (was always reappearing)\n\
             • Smaller, faster binary thanks to release build optimizations\n\
             • Hardened license state. No more crashes from a poisoned mutex on edge cases.",
        ),
        "1.6.0" => Some(
            "• 7-day Pro trial automatically starts on this update. Everything unlimited.\n\
             • After the trial, Free tier kicks in: 30 AI Polish/month, 20 dictionary entries, 50 history entries\n\
             • Existing entries above the cap are kept (you can keep using them, just can't add new ones)\n\
             • Activate a Pro license any time from Settings → TTP Pro to remove all limits forever",
        ),
        "1.5.0" => Some(
            "• Introducing TTP Pro. €17 once unlocks unlimited AI Polish, dictionary, and history.\n\
             • New Pro section in Settings to manage your license\n\
             • One-time payment, lifetime license, free updates forever\n\
             • Free tier remains fully usable",
        ),
        "1.3.5" => Some(
            "• Left-click on menu bar icon now opens the menu\n\
             • Tray menu shows \"Stop Recording\" during active recording\n\
             • Tutorial text adapts to your language (EN/FR)\n\
             • API key is validated before saving\n\
             • History capped at 500 entries\n\
             • Fixed Windows shortcut conflicts (Ctrl+Space is now default)\n\
             • Removed unused audio codec for faster builds",
        ),
        _ => None,
    }
}

/// Check whether a "What's New" popup should be shown.
/// Returns `Some((version, changelog))` if the app was updated since the user last dismissed,
/// or `None` if the user is already up-to-date.
#[command]
pub fn check_whats_new() -> Option<(String, String)> {
    let version = current_version();

    // Read last seen version (if the file doesn't exist, treat as "never seen")
    let last_seen = fs::read_to_string(last_seen_version_path())
        .ok()
        .map(|s| s.trim().to_string());

    // If the user already saw this version, nothing to show
    if last_seen.as_deref() == Some(version) {
        return None;
    }

    // Return changelog if one exists for the current version
    changelog_for(version).map(|log| (version.to_string(), log.to_string()))
}

/// Dismiss the "What's New" popup by recording the current version.
#[command]
pub fn dismiss_whats_new() -> Result<(), String> {
    let version = current_version();
    fs::write(last_seen_version_path(), version)
        .map_err(|e| format!("Failed to save last_seen_version: {}", e))
}
