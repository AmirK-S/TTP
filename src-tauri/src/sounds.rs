// TTP - Talk To Paste
// Sound effect playback for recording state transitions

use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use tauri::AppHandle;

// Embed sound files at compile time
// Using simple short beep tones - these are placeholder sounds
// Real sounds can be added later

// The house sounds — what plays for everyone, with no licence involved.
const START_SOUND: &[u8] = include_bytes!("../sounds/start.wav");
const STOP_SOUND: &[u8] = include_bytes!("../sounds/stop.wav");

/// Embedded bytes for a cosmetic sound pack, or `None` for the house pair.
///
/// Embedded rather than loaded from disk so a pack cannot go missing, and so
/// selecting one costs nothing at play time: this is a match on a `&str` and
/// a pointer, on the path between the user pressing the hotkey and hearing
/// that they were heard.
fn pack_bytes(pack_id: &str) -> Option<(&'static [u8], &'static [u8])> {
    // Assets are synthesised from scratch by scripts/synth_sounds.py — no
    // sample libraries, no licensing questions, and regenerable byte-for-byte.
    // An id with no entry here falls through to the house sounds rather than
    // going silent; see `unknown_pack_is_silent_not_missing`.
    macro_rules! pack {
        ($dir:literal) => {
            Some((
                include_bytes!(concat!("../sounds/packs/", $dir, "/start.wav")) as &[u8],
                include_bytes!(concat!("../sounds/packs/", $dir, "/stop.wav")) as &[u8],
            ))
        };
    }
    match pack_id {
        "radio" => pack!("radio"),
        "arcade" => pack!("arcade"),
        "submarine" => pack!("submarine"),
        "typewriter" => pack!("typewriter"),
        "bubble" => pack!("bubble"),
        _ => None,
    }
}

/// Resolve the start/stop pair to play right now.
///
/// Two layers of fallback, both deliberate: `cosmetics::effective_sound_pack`
/// refuses ids that are unknown or locked, and `pack_bytes` refuses ids whose
/// assets are not compiled in. Either way the user hears the house sounds.
/// There is no state in which a licence makes TTP go quiet.
fn active_pair() -> (&'static [u8], &'static [u8]) {
    let requested = crate::settings::get_settings().sound_pack;
    let pack = crate::cosmetics::effective_sound_pack(requested.as_deref());
    pack_bytes(pack.id).unwrap_or((START_SOUND, STOP_SOUND))
}

/// Play a sound from embedded bytes on a separate thread
fn play_sound_bytes(sound_data: &'static [u8]) {
    // Use Tauri's async runtime so rodio/cpal internals can find a Tokio
    // reactor. A bare `std::thread::spawn` panics with "there is no reactor
    // running". `spawn_blocking` runs on a runtime-attached worker thread
    // and matches the synchronous `sleep_until_end` body below.
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(source) = Decoder::new(Cursor::new(sound_data)) {
                if let Ok(sink) = Sink::try_new(&stream_handle) {
                    sink.append(source);
                    sink.sleep_until_end();
                }
            }
        }
    });
}

/// Play the recording start sound
pub fn play_start_sound(_app: &AppHandle) {
    play_sound_bytes(active_pair().0);
}

/// Play the recording stop sound
pub fn play_stop_sound(_app: &AppHandle) {
    play_sound_bytes(active_pair().1);
}

/// Play a pack's start sound on demand, so Settings can preview one before
/// the user commits to it. Ignores the licence deliberately: hearing what you
/// might buy is not the same as owning it, and a catalogue you cannot
/// audition is just a list of words.
#[tauri::command]
pub fn preview_sound_pack(pack_id: String) {
    let (start, _) = pack_bytes(&pack_id).unwrap_or((START_SOUND, STOP_SOUND));
    play_sound_bytes(start);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_pack_is_silent_not_missing() {
        // Guards a build where settings name a pack whose assets were never
        // compiled in. The user must still hear something.
        assert!(pack_bytes("no-such-pack").is_none());
    }

    #[test]
    fn house_sounds_are_embedded_and_non_empty() {
        assert!(!START_SOUND.is_empty());
        assert!(!STOP_SOUND.is_empty());
    }

    #[test]
    fn every_catalogued_pack_has_embedded_audio() {
        // The catalogue in `cosmetics` and the assets embedded here are two
        // lists that must not drift. A pack advertised in Settings that
        // silently plays the house sounds is worse than one that isn't
        // offered at all — the user would think they bought nothing.
        for pack in crate::cosmetics::SOUND_PACKS {
            if pack.free {
                continue;
            }
            assert!(
                pack_bytes(pack.id).is_some(),
                "pack '{}' is offered in Settings but has no embedded audio",
                pack.id
            );
        }
    }

    #[test]
    fn pack_audio_is_riff_wav() {
        // include_bytes! will happily embed anything. Check the magic so a
        // truncated or wrong-format asset fails the build rather than
        // failing silently in rodio at runtime.
        for pack in crate::cosmetics::SOUND_PACKS {
            let Some((start, stop)) = pack_bytes(pack.id) else {
                continue;
            };
            for (label, bytes) in [("start", start), ("stop", stop)] {
                assert!(bytes.len() > 44, "{} {} is too short to be a WAV", pack.id, label);
                assert_eq!(&bytes[0..4], b"RIFF", "{} {} is not RIFF", pack.id, label);
                assert_eq!(&bytes[8..12], b"WAVE", "{} {} is not WAVE", pack.id, label);
            }
        }
    }
}
