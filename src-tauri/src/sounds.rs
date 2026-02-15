// TTP - Talk To Paste
// Sound effect playback for recording state transitions

use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use tauri::AppHandle;

// Embed sound files at compile time
// Using simple short beep tones - these are placeholder sounds
// Real sounds can be added later

// A simple 440Hz sine wave beep (start sound - higher pitch)
const START_SOUND: &[u8] = include_bytes!("../sounds/start.wav");

// A simple 330Hz sine wave beep (stop sound - lower pitch)
const STOP_SOUND: &[u8] = include_bytes!("../sounds/stop.wav");

/// Play a sound from embedded bytes on a separate thread
fn play_sound_bytes(sound_data: &'static [u8]) {
    std::thread::spawn(move || {
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
    play_sound_bytes(START_SOUND);
}

/// Play the recording stop sound
pub fn play_stop_sound(_app: &AppHandle) {
    play_sound_bytes(STOP_SOUND);
}
