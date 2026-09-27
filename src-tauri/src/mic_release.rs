// TTP - Talk To Paste
// Asks CoreAudio, after every capture is torn down, whether the microphone
// really went off.
//
// Every other capture check in the trace (`capture.orphan_*`,
// `capture.stale_dropped`, the arbiter) reads TTP's own bookkeeping: they
// prove TTP *dropped* the stream, not that the operating system closed it.
// On 2026-09-27 those two came apart. cpal 0.15.3 gave any stream opened on a
// device picked by name (not the OS default) a disconnect listener holding a
// strong reference back to the stream, so dropping it freed nothing: each
// dictation on a pinned "MacBook Air Microphone" left one more AudioUnit
// running, the orange dot stayed on, and the trace showed eight clean
// `capture.start` / `capture.stop` pairs. cpal 0.16 fixed the cycle; this
// module is the check that would have caught it on the first dictation.
//
// The question is per-process — "is TTP running input?" — not per-device,
// so another app using the same microphone (a call, a recorder) is never
// mistaken for a leak.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Bumped every time a capture stream is about to be built. A check that
/// finds it moved since its drop knows a newer dictation owns the
/// microphone now, and that "input is running" is that dictation, not a leak.
static STREAMS_BUILT: AtomicU64 = AtomicU64::new(0);

/// First look after the drop. CoreAudio stops the unit synchronously; this is
/// margin for the HAL to publish it.
const FIRST_LOOK_MS: u64 = 300;
/// Second look, only when the first still saw input running: long enough that
/// a slow Bluetooth teardown is not reported as a leak.
const SECOND_LOOK_MS: u64 = 2_000;

/// Call immediately before building a capture stream.
pub fn note_stream_built() {
    STREAMS_BUILT.fetch_add(1, Ordering::SeqCst);
}

/// Call right after a capture stream has been dropped. `site` names the path
/// that dropped it (`stop`, `orphan_prevented`, `orphan_reclaimed`,
/// `stale_dropped`). Returns immediately; the check runs on its own thread.
pub fn verify_after_drop(site: &'static str) {
    let generation = STREAMS_BUILT.load(Ordering::SeqCst);
    let device = crate::audio_capture::last_capture_device();
    let spawned = std::thread::Builder::new()
        .name("ttp-mic-release".into())
        .spawn(move || check(site, generation, device));
    if let Err(e) = spawned {
        crate::trace::degraded(
            "capture.mic_release",
            serde_json::json!({ "site": site, "error": format!("thread spawn: {}", e) }),
        );
    }
}

enum Look {
    Off,
    Live,
    /// A newer capture started after the drop; the answer would be about it.
    Superseded,
    /// CoreAudio could not answer (before macOS 14, or not a Mac).
    Unknown(String),
}

fn look(generation: u64) -> Look {
    if STREAMS_BUILT.load(Ordering::SeqCst) != generation {
        return Look::Superseded;
    }
    match input_running_in_this_process() {
        Ok(true) => Look::Live,
        Ok(false) => Look::Off,
        Err(e) => Look::Unknown(e),
    }
}

fn check(site: &'static str, generation: u64, device: Option<String>) {
    std::thread::sleep(Duration::from_millis(FIRST_LOOK_MS));
    let (result, after_ms) = match look(generation) {
        Look::Live => {
            std::thread::sleep(Duration::from_millis(SECOND_LOOK_MS - FIRST_LOOK_MS));
            (look(generation), SECOND_LOOK_MS)
        }
        other => (other, FIRST_LOOK_MS),
    };
    match result {
        Look::Off => crate::trace::event(
            "capture.mic_released",
            serde_json::json!({ "site": site, "released": true, "after_ms": after_ms }),
        ),
        Look::Superseded => crate::trace::event(
            "capture.mic_released",
            serde_json::json!({ "site": site, "released": null, "reason": "newer_capture", "after_ms": after_ms }),
        ),
        Look::Unknown(e) => crate::trace::event(
            "capture.mic_released",
            serde_json::json!({ "site": site, "released": null, "reason": e, "after_ms": after_ms }),
        ),
        Look::Live => {
            crate::logging::log_error(&format!(
                "[AudioCapture] microphone still live {} ms after the capture was dropped ({}, device {:?}) — the stream leaked below TTP",
                after_ms, site, device
            ));
            crate::trace::event(
                "capture.mic_still_live",
                serde_json::json!({ "site": site, "device": device, "after_ms": after_ms }),
            );
        }
    }
}

/// Whether CoreAudio has input running for this process right now.
#[cfg(target_os = "macos")]
fn input_running_in_this_process() -> Result<bool, String> {
    use std::ffi::c_void;

    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyData(
            object: u32,
            address: *const AudioObjectPropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    const SYSTEM_OBJECT: u32 = 1;
    const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
    const ELEMENT_MAIN: u32 = 0;
    // kAudioHardwarePropertyTranslatePIDToProcessObject, macOS 14+.
    const TRANSLATE_PID: u32 = u32::from_be_bytes(*b"id2p");
    // kAudioProcessPropertyIsRunningInput, macOS 14+.
    const IS_RUNNING_INPUT: u32 = u32::from_be_bytes(*b"piri");

    let pid = std::process::id() as i32;
    let mut process: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let address = AudioObjectPropertyAddress { selector: TRANSLATE_PID, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    // SAFETY: every pointer is to a live local of the size passed with it.
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &address,
            std::mem::size_of::<i32>() as u32,
            &pid as *const i32 as *const c_void,
            &mut size,
            &mut process as *mut u32 as *mut c_void,
        )
    };
    if status != 0 {
        return Err(format!("pid_lookup_status_{}", status));
    }
    // The HAL has no process object for us: nothing of ours is running.
    if process == 0 {
        return Ok(false);
    }

    let mut running: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let address = AudioObjectPropertyAddress { selector: IS_RUNNING_INPUT, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    // SAFETY: as above.
    let status = unsafe {
        AudioObjectGetPropertyData(
            process,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut running as *mut u32 as *mut c_void,
        )
    };
    if status != 0 {
        return Err(format!("running_input_status_{}", status));
    }
    Ok(running != 0)
}

#[cfg(not(target_os = "macos"))]
fn input_running_in_this_process() -> Result<bool, String> {
    Err("unsupported_os".into())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// Talks to the real HAL. A test process has opened no input, so the
    /// answer must be a clean "off" — an error here means the FFI is wrong.
    #[test]
    fn a_process_with_no_capture_reads_as_off() {
        assert_eq!(input_running_in_this_process(), Ok(false));
    }

    #[test]
    fn a_newer_capture_supersedes_the_check() {
        let generation = STREAMS_BUILT.load(Ordering::SeqCst);
        note_stream_built();
        assert!(matches!(look(generation), Look::Superseded));
    }
}
