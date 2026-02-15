// TTP - Clipboard management
// Handles clipboard read/write with preservation of original content
//
// Per CONTEXT.md: "User expects clipboard to always have the transcription as backup"
// We write text to clipboard BEFORE paste attempt, and only restore on SUCCESS.
// On failure, text stays in clipboard for manual paste.

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Guard that preserves original clipboard content and can restore it
///
/// Usage pattern:
/// 1. Create guard (saves current clipboard)
/// 2. Write transcription text to clipboard
/// 3. Attempt paste simulation
/// 4. If paste succeeds, restore original clipboard
/// 5. If paste fails, leave transcription in clipboard for manual paste
pub struct ClipboardGuard {
    /// Original clipboard content (if any)
    original_content: Option<String>,
    /// App handle for clipboard access
    app: AppHandle,
}

impl ClipboardGuard {
    /// Create a new ClipboardGuard, saving the current clipboard content
    pub fn new(app: &AppHandle) -> Self {
        // Save current clipboard content
        // read_text() returns Result<String, Error>, .ok() converts to Option<String>
        let original_content = app.clipboard().read_text().ok();

        Self {
            original_content,
            app: app.clone(),
        }
    }

    /// Write text to the clipboard
    pub fn write_text(&self, text: &str) -> Result<(), String> {
        self.app
            .clipboard()
            .write_text(text)
            .map_err(|e| format!("Failed to write to clipboard: {}", e))
    }

    /// Restore the original clipboard content
    /// Only call this after successful paste - on failure, leave transcription in clipboard
    pub fn restore(self) -> Result<(), String> {
        if let Some(original) = self.original_content {
            self.app
                .clipboard()
                .write_text(&original)
                .map_err(|e| format!("Failed to restore clipboard: {}", e))?;
        }
        Ok(())
    }
}
