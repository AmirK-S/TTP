//! The panic hook is installed for a DEFAULT user, and it fires.
//!
//! This is the other half of `tests/abort_observability.rs`, and the two are
//! deliberately kept apart.
//!
//! `abort_observability.rs` answers "does a synchronous append from a panic
//! hook survive `panic = \"abort\"`?" — by compiling and running a real
//! `-C panic=abort` process, because a test binary unwinds and an assertion
//! made here about survival would be the exact hazard
//! `docs/engineering-standards.md` §1.8 names: a test that passes while
//! proving nothing about the shipped product. It says nothing about this app's
//! hook, because its probes do not contain this app.
//!
//! This file answers the question that was actually broken, and that no amount
//! of evidence about the panic strategy could have answered: **is there a hook
//! at all?** Until now, for a default user, there was not. The app's only
//! `std::panic::set_hook` sat inside `if telemetry_active`, and
//! `telemetry_enabled` defaults to false, so a panic on a default install
//! produced no Sentry event, no trace line, and not even a stderr print. The
//! abort was invisible — and it would have stayed invisible under `unwind`.
//!
//! So the assertion below is: with telemetry OFF, `install_panic_hook_with`
//! produces a hook that runs on panic, on the main thread and on a worker
//! thread, and hands a well-formed `app.panic` record to the sink. In
//! production that sink is `logging::log_trace_line`, whose write shape is the
//! one the other file proved survivable. Neither test borrows the other's
//! evidence; together they cover the path end to end.
//!
//! Run: `cargo test --test panic_hook`

use std::sync::Mutex;

/// Everything the hook handed to its sink, in order.
static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Stands in for `logging::log_trace_line`, which has the same signature. It
/// is a plain `fn` and not a closure because the hook takes a function
/// pointer: a hook that could capture arbitrary state would be a hook that
/// could hold a lock the panicking thread was already holding.
fn capture(line: &str) -> bool {
    CAPTURED.lock().expect("capture mutex").push(line.to_string());
    true
}

/// One test, not three, on purpose: `set_hook` is process-global state, and
/// two tests installing hooks in the same binary would race on it. Cargo runs
/// test functions concurrently by default, so the sequencing has to come from
/// them being one function.
#[test]
fn the_hook_fires_with_telemetry_off_on_every_thread() {
    // false = the default user. This is the whole point of the test.
    ttp_lib::install_panic_hook_with(false, capture);

    // 1. Main thread.
    let caught = std::panic::catch_unwind(|| {
        panic!("deliberate: default-consent main-thread panic");
    });
    assert!(caught.is_err(), "the probe panic did not happen");

    // 2. A named worker, the shape the paste injection runs in
    //    (`spawn_blocking` + `catch_unwind` in `pipeline.rs`). A hook that only
    //    covered the main thread would miss the one panic this app has a
    //    documented theory about.
    let worker = std::thread::Builder::new()
        .name("blocking-worker".into())
        .spawn(|| panic!("deliberate: worker panic"))
        .expect("spawn worker");
    assert!(worker.join().is_err(), "the worker did not panic");

    let lines = CAPTURED.lock().expect("capture mutex").clone();
    assert_eq!(
        lines.len(),
        2,
        "expected one record per panic, got {:?}",
        lines
    );

    for line in &lines {
        assert!(
            line.contains(" app.panic "),
            "not an app.panic record: {:?}",
            line
        );
        // The trace's standalone-id column, because a panic belongs to no
        // dictation. Asserted here so a future change that starts attaching a
        // fabricated trace id has to be deliberate.
        assert!(
            line.contains("[········]"),
            "a panic must not claim a dictation id: {:?}",
            line
        );
        // One record, one physical line: `logging::append_line` writes the
        // line and its newline as a single `write_all`, and an embedded
        // newline would tear the record from the inside.
        assert!(
            !line.contains('\n'),
            "the record spans more than one line: {:?}",
            line
        );
        // The line/column of the `panic!` is the first thing anyone reading a
        // crash report needs.
        assert!(
            line.contains("tests/panic_hook.rs:"),
            "the record does not say where the panic was: {:?}",
            line
        );
    }

    assert!(
        lines[0].contains("main-thread panic"),
        "first record is not the main-thread panic: {:?}",
        lines[0]
    );
    assert!(
        lines[1].contains(r#""thread":"blocking-worker""#),
        "the worker's record does not name its thread: {:?}",
        lines[1]
    );
}
