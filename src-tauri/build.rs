fn main() {
    // Embed git commit SHA at build time so the Settings → Updates panel can
    // display the exact build the user is running. Useful for distinguishing
    // beta builds that share a marketing version string (e.g. "3.0.0" for
    // all v3.0.0-beta.N tags since the manifest doesn't include the
    // pre-release suffix — Windows MSI rejects non-numeric pre-releases).
    //
    // Resolution order:
    //   1. GIT_COMMIT_SHA env var (CI passes this from `${{ github.sha }}`)
    //   2. `git rev-parse --short=7 HEAD` (local dev builds)
    //   3. literal "unknown" fallback
    let sha = std::env::var("GIT_COMMIT_SHA")
        .ok()
        .map(|s| s.chars().take(7).collect::<String>())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short=7", "HEAD"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=TTP_BUILD_SHA={}", sha);

    // Trigger a rebuild on commit so SHA stays fresh during local dev.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_SHA");

    tauri_build::build()
}
