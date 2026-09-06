//! What survives `panic = "abort"`, established by running a real one.
//!
//! `[profile.release]` sets `panic = "abort"` (`Cargo.toml`). `[profile.dev]`
//! does not. That asymmetry is the reason this file exists and the reason it
//! does not simply `panic!()` and look at the result: a test that panics under
//! `cargo test` observes UNWINDING, which is not what the shipped binary does.
//! `docs/engineering-standards.md` §1.8 names that exact hazard — "a test
//! written against those arms would pass while proving nothing about the
//! product". Asserting on an unwind here would be another instance of it.
//!
//! So the subject is not this binary. Each test compiles a small program with
//! `rustc -C panic=abort`, runs it, and asserts on what that process left on
//! disk and how it died. Everything below is therefore PROVEN under the
//! release panic strategy, not inferred from it.
//!
//! Three facts are established, and each one decides something:
//!
//! 1. **`std::panic::set_hook` runs before the abort.** So a panic hook is a
//!    usable place to record that the process is about to die — which is what
//!    makes the abort observable without touching the release profile.
//!
//! 2. **A synchronous `open(O_APPEND)` + one `write_all` from inside the hook
//!    reaches the file.** No `fsync` is needed: `write(2)` hands the bytes to
//!    the page cache and process death does not take them back. This is
//!    exactly the shape of `logging::append_line`, which is also lock-free —
//!    it opens, writes once, and drops — so calling it from a panic hook
//!    cannot deadlock on a mutex the panicking thread was already holding.
//!
//! 3. **A push onto a channel served by a background writer thread does NOT
//!    reach the file.** This is the one that matters, because commit e9449e0
//!    moved the trace onto exactly that shape (`trace.rs`: bounded
//!    `sync_channel` + a `ttp-trace-writer` thread). `trace::stage(...)` from
//!    a panic hook enqueues a line into a thread that the abort will never
//!    schedule again. The obvious fix is the broken one.
//!
//! Consequence for `transcription/pipeline.rs`: the `Ok(Err(_))` arm after
//! `catch_unwind` is unreachable in release. Test 3's sibling below proves it
//! directly — under `-C panic=abort` the closure's panic never returns control
//! to `catch_unwind` at all.
//!
//! Run: `cargo test --test abort_observability`

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the probe programs and their output live for one test.
///
/// Deliberately not `tempfile`: this crate has no dev-dependencies and adding
/// one to a Tauri app's build graph to get a directory name is not a trade
/// worth making.
fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("ttp-abort-{}-{}-{}", tag, std::process::id(), nanos));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Compile `source` with `-C panic=abort` and return the binary's path.
///
/// Returns `None` when `rustc` is not on PATH. A missing toolchain is not a
/// failing assertion — it means the property was not checked, and saying so is
/// the honest outcome. (In practice `cargo test` implies `rustc`, so this is a
/// belt-and-braces branch, not an expected one.)
fn build_abort_probe(dir: &Path, source: &str) -> Option<PathBuf> {
    let src_path = dir.join("probe.rs");
    std::fs::write(&src_path, source).expect("write probe source");
    let bin_path = dir.join("probe");

    let out = Command::new("rustc")
        .arg("-C")
        .arg("panic=abort")
        .arg("-o")
        .arg(&bin_path)
        .arg(&src_path)
        .output()
        .ok()?;

    assert!(
        out.status.success(),
        "probe failed to compile:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(bin_path)
}

/// The lock-free append the panic hook is allowed to perform, copied here so
/// the probe is self-contained. Byte-for-byte the shape of
/// `logging::append_line`: create+append open, ONE `write_all` of record and
/// newline together, drop. See `logging::frame_record` for why the newline
/// must be in the same buffer.
const APPEND_FN: &str = r#"
use std::fs::OpenOptions;
use std::io::Write;
fn append(path: &str, line: &str) -> bool {
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => {
            let mut buf = line.as_bytes().to_vec();
            buf.push(b'\n');
            f.write_all(&buf).is_ok()
        }
        Err(_) => false,
    }
}
"#;

/// Did the process die the way `panic = "abort"` dies?
///
/// On unix that is SIGABRT (6). Asserted rather than assumed, because a probe
/// that exited cleanly would mean the panic never happened and every other
/// assertion in the test would be vacuously satisfied by an untouched file.
fn assert_died_by_abort(status: std::process::ExitStatus) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.signal(),
            Some(6),
            "probe did not die by SIGABRT — it exited {:?}, so it did not abort and proves nothing",
            status
        );
    }
    #[cfg(not(unix))]
    {
        assert!(!status.success(), "probe exited cleanly instead of aborting");
    }
}

#[test]
fn a_panic_hook_runs_and_its_synchronous_write_survives_the_abort() {
    let dir = scratch("sync");
    let out_path = dir.join("trace.txt");
    let source = format!(
        r#"{append}
fn main() {{
    let path = std::env::args().nth(1).unwrap();
    std::panic::set_hook(Box::new(move |info| {{
        let loc = info.location().map(|l| l.line()).unwrap_or(0);
        append(&path, &format!("panic.abort line={{}}", loc));
    }}));
    panic!("deliberate");
}}
"#,
        append = APPEND_FN
    );

    let Some(bin) = build_abort_probe(&dir, &source) else {
        eprintln!("SKIPPED: rustc not on PATH — abort survivability NOT checked");
        return;
    };

    let status = Command::new(&bin)
        .arg(out_path.to_str().unwrap())
        .status()
        .expect("run probe");
    assert_died_by_abort(status);

    let written = std::fs::read_to_string(&out_path).unwrap_or_default();
    assert!(
        written.contains("panic.abort"),
        "the hook's synchronous write did not reach the file; got {:?}",
        written
    );
    assert!(
        written.ends_with('\n'),
        "the record landed without its newline — the next append would share its line"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_channel_push_from_the_hook_does_not_survive_the_abort() {
    // The negative case, and the reason this file was written. `trace::emit`
    // is a `try_send` onto a bounded channel drained by a named writer thread.
    // The probe reproduces that shape, including the writer doing real work
    // (the live one does a stat, a rotation check, an open and a write) before
    // its first byte lands. The sleep makes the race deterministic instead of
    // leaving the assertion to the scheduler: a flaky proof of a safety
    // property is worse than none.
    let dir = scratch("chan");
    let out_path = dir.join("trace.txt");
    let source = format!(
        r#"{append}
use std::sync::mpsc::sync_channel;
fn main() {{
    let path = std::env::args().nth(1).unwrap();
    let (tx, rx) = sync_channel::<String>(4096);
    let writer_path = path.clone();
    std::thread::spawn(move || {{
        while let Ok(line) = rx.recv() {{
            std::thread::sleep(std::time::Duration::from_millis(200));
            append(&writer_path, &line);
        }}
    }});
    std::panic::set_hook(Box::new(move |_| {{
        let _ = tx.send("panic.abort {{}}".to_string());
    }}));
    panic!("deliberate");
}}
"#,
        append = APPEND_FN
    );

    let Some(bin) = build_abort_probe(&dir, &source) else {
        eprintln!("SKIPPED: rustc not on PATH — abort survivability NOT checked");
        return;
    };

    let status = Command::new(&bin)
        .arg(out_path.to_str().unwrap())
        .status()
        .expect("run probe");
    assert_died_by_abort(status);

    assert!(
        !out_path.exists(),
        "the queued line reached the file after all — if the trace writer has \
         become synchronous again, the queue is back on the dictation path and \
         trace.rs's first stated property is broken"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn catch_unwind_never_returns_when_the_panic_aborts() {
    // `pipeline.rs` wraps the paste injection in `spawn_blocking` +
    // `catch_unwind` and handles `Ok(Err(_))` with a `paste.result
    // {"kind":"panic"}` trace line. Under the release profile that arm cannot
    // run, and this proves it rather than citing the reference: the probe
    // panics inside `catch_unwind` on a NON-main thread — the same shape as a
    // blocking worker — and records whether control ever came back.
    //
    // The hook fires (so the panic really happened), the process dies, and the
    // line after `catch_unwind` is never written.
    let dir = scratch("worker");
    let out_path = dir.join("trace.txt");
    let source = format!(
        r#"{append}
fn main() {{
    let path = std::env::args().nth(1).unwrap();
    let hook_path = path.clone();
    std::panic::set_hook(Box::new(move |_| {{
        append(&hook_path, "hook.ran");
    }}));
    let worker = std::thread::Builder::new()
        .name("blocking-worker".into())
        .spawn(move || {{
            let r = std::panic::catch_unwind(|| {{ panic!("inside paste"); }});
            append(&path, &format!("catch_unwind.returned err={{}}", r.is_err()));
        }})
        .unwrap();
    let _ = worker.join();
}}
"#,
        append = APPEND_FN
    );

    let Some(bin) = build_abort_probe(&dir, &source) else {
        eprintln!("SKIPPED: rustc not on PATH — abort survivability NOT checked");
        return;
    };

    let status = Command::new(&bin)
        .arg(out_path.to_str().unwrap())
        .status()
        .expect("run probe");
    assert_died_by_abort(status);

    let written = std::fs::read_to_string(&out_path).unwrap_or_default();
    assert!(
        written.contains("hook.ran"),
        "the hook did not run on the worker thread; got {:?}",
        written
    );
    assert!(
        !written.contains("catch_unwind.returned"),
        "catch_unwind RETURNED under panic=abort — the release profile is no \
         longer aborting, and pipeline.rs's panic arm would now be live code. \
         Got {:?}",
        written
    );
    let _ = std::fs::remove_dir_all(&dir);
}
