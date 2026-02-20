// TTP - Talk To Paste
// Application state management

use serde::{Deserialize, Serialize};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

pub struct AppState {
    pub recording_state: RecordingState,
    pub hands_free_mode: bool,
    pub last_shortcut_time: Option<Instant>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording_state: RecordingState::Idle,
            hands_free_mode: false,
            last_shortcut_time: None,
        }
    }
}

impl AppState {
    pub fn set_state(&mut self, state: RecordingState, app: &AppHandle) {
        let old_state = self.recording_state.clone();
        self.recording_state = state.clone();

        // Start/stop audio level monitoring for pill wave visualization
        match &state {
            RecordingState::Recording => crate::audio_monitor::start(app.clone()),
            _ => crate::audio_monitor::stop(),
        }

        // Handle pill visibility when transitioning to Idle
        // When going from Processing/Recording to Idle, respect hide_pill_when_inactive setting
        if old_state != RecordingState::Idle && state == RecordingState::Idle {
            if !crate::tray::should_show_pill(app) {
                crate::tray::hide_pill(app);
            }
        }

        app.emit("recording-state-changed", &state).ok();
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.recording_state, RecordingState::Recording)
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.recording_state, RecordingState::Idle)
    }
}
