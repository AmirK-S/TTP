// TTP - Talk To Paste
// "What's New" version tracking — shows changelog after app update

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
        "2.0.2" => Some(
            "🎉 TTP 2.0.2\n\
             • TTP now speaks French — auto-detects your system language, switch any time from Settings → Language. UI, tray menu, and OS notifications all flip together.\n\
             • Self-update finally relaunches the app cleanly on macOS (was silently failing on betas).\n\
             • Gatekeeper no longer re-prompts after an update — the quarantine flag is stripped post-install.\n\
             • Tray icon shows a red dot when Input Monitoring permission is missing — one-click deep link to System Settings.\n\
             • macOS function keys (F3 / F4 / F6 / Mission Control / Launchpad) can no longer trigger recordings even when held.\n\
             • Symbolicated crash reports — future panics resolve to real function names instead of raw addresses.\n\
             • Internal: three remaining Tokio runtime call sites hardened to prevent the \"no reactor running\" panic some users hit on macOS 26.",
        ),
        "2.0.2-beta.9" => Some(
            "🔬 Defensive cleanup + symbolicated crash reports\n\
             • Three remaining raw `tokio::task::spawn_blocking` / `std::thread::spawn` call sites converted to `tauri::async_runtime::spawn_blocking` (paste pipeline, dictionary detection poll, post-update xattr cleanup). May or may not address the recurring `no reactor running` panic — telemetry will tell.\n\
             • CI now uploads debug symbols (.dSYM/.pdb) to Sentry on every release. Future fatal stacks will resolve to real Rust function names instead of `__mh_execute_header`.",
        ),
        "2.0.2-beta.8" => Some(
            "🧪 Validation build for the Restart-After-Update fix\n\
             • No code change — pure version bump used to verify the new LaunchServices-based restart path in beta.7 works end-to-end.\n\
             • If the \"Restart Now\" button on this update successfully relaunches the app, the fix is confirmed.",
        ),
        "2.0.2-beta.7" => Some(
            "🛠 Restart-after-update fixed (for real this time)\n\
             • \"Restart Now\" after an update now uses macOS LaunchServices (`open -n -a`) instead of the standard plugin-process relaunch — the old path was silently failing for beta builds: the current process died but the new one never appeared.\n\
             • Added the missing window:set_title permission so dynamic window titles stop throwing an ACL error on every window mount (Sentry TTP-C).\n\
             • No new features — pure post-launch cleanup driven by Sentry telemetry.",
        ),
        "2.0.2-beta.6" => Some(
            "🌍 TTP now speaks French\n\
             • Auto-detects your system language at first launch (français on a French macOS, English otherwise)\n\
             • Switch any time from Settings → Language — every window, the tray menu, and OS notifications all flip together\n\
             • Tutoiement throughout the French UI, with the same glossary as the landing page\n\
             • Errors emitted by the Rust backend now flow as translation codes so the pill and notifications speak your language too\n\
             • Internal: parity check in CI guards against missing translations; <html lang> set for screen-readers\n\
             • CI: Windows builds no longer blocked by the MSI bundler on beta versions (NSIS .exe still ships)",
        ),
        "1.8.1" => Some(
            "🚀 Launch polish\n\
             • Onboarding now waits for your Groq API key before closing — no more landing in the app with nothing configured.\n\
             • Input Monitoring permission (used for the Fn key) is now requested in onboarding instead of as a surprise system prompt afterwards.\n\
             • Clearer Groq sign-up steps and a reassurance that the free tier covers tens of thousands of transcriptions a month.\n\
             • Webview locked down with a strict Content Security Policy — XSS in the frontend can no longer reach external networks.\n\
             • Future panic reports will include real symbol names (we now upload debug info to Sentry on every release).",
        ),
        "1.8.0" => Some(
            "🔒 Security release\n\
             • Your Groq API key is now stored in the macOS Keychain (Windows Credential Manager) instead of a plaintext file. Existing keys migrate automatically on first launch.\n\
             • License and trial state are now signed with a per-machine secret stored in the keychain — a forged file from one Mac can no longer be replayed on another.\n\
             • Constant-time signature comparison closes a small timing side-channel.\n\
             • Email addresses in error reports are now redacted before they leave your machine.\n\
             • Fixed a rare panic on transcription completion (\"no reactor running\") seen by a single user on macOS 26.3.1.\n\
             • Quality-of-life: more Sentry detail on transcription failures, faster Settings open, smaller settings.json recovery on corruption.",
        ),
        "1.7.3" => Some(
            "• Fix: a slow IPC bridge boot could leave the app in a state where every backend call failed for the rest of the session — now it retries instead of caching the failure\n\
             • Bumped the transcription rate limit from 5 to 20 per minute so power users don't hit it on a normal workflow (still caps a runaway loop)\n\
             • Settings file now keeps a .bak so a half-written settings.json can recover instead of resetting your preferences\n\
             • Internal: smaller polish from production telemetry feedback",
        ),
        "1.7.2" => Some(
            "• Faster Settings: history and dictionary now cache in memory (5s TTL) instead of re-reading the file on every render — opening Settings feels instant, even with hundreds of entries\n\
             • Smoother dictionary list: rows skip re-render when their entry hasn't changed (real win once you've got a few dozen)\n\
             • Better error triage on our side: transcription failures now report the HTTP status code separately so we can spot a Groq outage faster\n\
             • Internal: hardened the release pipeline (auto-retry on Apple notarization flakes, mandatory signing for releases, pinned action versions)",
        ),
        "1.7.1" => Some(
            "• Fixed F3 / F4 / F6 still triggering recording on macOS — the keys macOS hijacks for Mission Control, Launchpad, and Do Not Disturb were slipping past the previous filter\n\
             • Fixed the \"What's New\" popup growing too tall to show its dismiss button when the changelog ran long\n\
             • Anti-abuse: capped transcription requests at 5 per minute so a runaway loop can't burn through your Groq credits\n\
             • Hardened cold-start: the IPC bridge is now waited on before any backend call, eliminating a class of crashes that hit some users on launch\n\
             • Smaller initial JS payload thanks to code-splitting (Tauri + icon library load on demand)\n\
             • Internal: cleaner timer lifecycle in the transcription and updater hooks; small dead-code cleanup",
        ),
        "1.7.0" => Some(
            "• Onboarding refresh: clearer help text, ordered steps, trial banner, and one-tap link to System Settings when a permission is denied\n\
             • Friendlier error messages when Groq is rate-limited (429) or your API key is invalid (401) — no more raw HTTP codes in the pill\n\
             • You now get a quick \"Polish unavailable — pasting raw text\" notice instead of silent fallback\n\
             • Microphone permission denied mid-session now shows feedback in the pill instead of failing silently\n\
             • Update failures (check + download) are tracked so we can spot regressions; the \"Check for Updates\" button no longer gets stuck after a transient network blip\n\
             • macOS: the quarantine flag is cleared after a self-update so Gatekeeper stops re-prompting you\n\
             • Anti-tamper: license and usage caches are now HMAC-signed; manual file edits no longer hand out free Pro\n\
             • Fewer ways the app can crash on cold start (license cache poison recovery, recordings dir errors)\n\
             • Crash reporter can no longer take itself down on its own regex (defensive)\n\
             • Privacy: audio-backup failures send only a category — no more raw OS paths\n\
             • Removed listener race condition that could surface as a UI freeze (TTP-5)\n\
             • Background analytics errors fixed (TTP-6/8)",
        ),
        "1.6.2" => Some(
            "• Fixed background analytics errors that were spamming our crash reporter\n\
             • Hardened event listeners against a rare race that could surface as a UI freeze\n\
             • Crash reporter pipeline can no longer panic on its own regex (defensive)\n\
             • Privacy: audio-backup failures send a category instead of the raw OS path\n\
             • No new features — pure stability pass driven by production telemetry",
        ),
        "1.6.1" => Some(
            "• Launch at startup — toggle in Settings to open TTP automatically when you log in\n\
             • Fixed F3 / F6 / other system function keys triggering recordings on macOS\n\
             • Pill visibility setting is now respected after restart (was always reappearing)\n\
             • Smaller, faster binary thanks to release build optimizations\n\
             • Hardened license state — no more crashes from a poisoned mutex on edge cases",
        ),
        "1.6.0" => Some(
            "• 7-day Pro trial automatically starts on this update — enjoy everything unlimited\n\
             • After the trial, Free tier kicks in: 30 AI Polish/month, 20 dictionary entries, 50 history entries\n\
             • Existing entries above the cap are kept (you can keep using them, just can't add new ones)\n\
             • Activate a Pro license any time from Settings → TTP Pro to remove all limits forever",
        ),
        "1.5.0" => Some(
            "• Introducing TTP Pro — unlock unlimited AI Polish, dictionary, and history\n\
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
