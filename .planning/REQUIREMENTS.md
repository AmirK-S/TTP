# Requirements: TTP (Talk To Paste)

**Defined:** 2025-01-29
**Core Value:** One shortcut to turn speech into text, anywhere.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Core Recording

- [x] **REC-01**: App runs as menu bar icon (macOS) / system tray icon (Windows)
- [x] **REC-02**: User can trigger recording via global keyboard shortcut
- [x] **REC-03**: Push-to-talk mode: hold shortcut = record, release = stop and transcribe
- [x] **REC-04**: Double-tap shortcut toggles persistent recording (tap again to stop)
- [x] **REC-05**: Visual indicator shows recording state (icon change + optional overlay)
- [x] **REC-06**: Audio captured from system default microphone

### Transcription

- [x] **TRX-01**: Audio sent to OpenAI Whisper API for transcription
- [x] **TRX-02**: Transcription includes proper punctuation
- [x] **TRX-03**: Multi-language support (Whisper handles 100+ languages)

### AI Polish

- [x] **POL-01**: Transcription processed by GPT-4o-mini for cleanup
- [x] **POL-02**: Filler words removed (um, uh, like, you know)
- [x] **POL-03**: Grammar corrected while preserving meaning
- [x] **POL-04**: Self-corrections handled ("Tuesday, no wait, Wednesday" -> "Wednesday")
- [x] **POL-05**: User dictionary terms injected as context for correct spelling

### Output

- [x] **OUT-01**: Polished text auto-pasted into active text field
- [x] **OUT-02**: Fallback to clipboard + notification when auto-paste impossible
- [x] **OUT-03**: Original clipboard content preserved (not overwritten permanently)

### Learning System

- [x] **LRN-01**: System detects user corrections after auto-paste
- [x] **LRN-02**: Corrections stored locally as dictionary entries
- [x] **LRN-03**: Future transcriptions use learned corrections for AI polish context
- [x] **LRN-04**: User can view/edit learned corrections in settings

### Configuration

- [x] **CFG-01**: First-run setup prompts for OpenAI API key
- [x] **CFG-02**: API key stored securely in system keychain/credential store
- [x] **CFG-03**: Settings UI accessible from menu bar/tray menu
- [x] **CFG-04**: User can customize global shortcut
- [x] **CFG-05**: User can enable/disable AI polish (raw transcription option)

### Platform Support

- [x] **PLT-01**: Native macOS app (menu bar, accessibility permissions)
- [x] **PLT-02**: Native Windows app (system tray, no special permissions needed)
- [x] **PLT-03**: Consistent feature parity across platforms

### Transcription History

- [x] **HST-01**: All transcriptions saved locally with timestamps
- [x] **HST-02**: User can view recent transcription history
- [x] **HST-03**: User can copy any past transcription

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Offline Mode

- **OFF-01**: Local Whisper model for offline transcription
- **OFF-02**: Graceful degradation when internet unavailable
- **OFF-03**: Model download/management UI

### Advanced Features

- **ADV-01**: Context-aware formatting based on active app
- **ADV-02**: Voice commands for editing ("delete that", "make it formal")
- **ADV-03**: Snippets/text expansion via voice shortcuts
- **ADV-04**: Cross-device sync of dictionary and settings

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Always-on listening | Privacy nightmare, battery drain, accidental triggers |
| Subscription pricing | User wants BYOK model only |
| Meeting transcription | Different product category |
| Audio file import | Different use case (MacWhisper owns this) |
| Real-time streaming transcription | Latency issues, partial results confusing |
| Speaker identification | Wrong product category, single-user dictation |
| Browser extensions | Scope creep, system-level works everywhere |
| Manual dictionary entry | Learning from corrections is more elegant |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| REC-01 | Phase 1 | Complete |
| REC-02 | Phase 1 | Complete |
| REC-03 | Phase 1 | Complete |
| REC-04 | Phase 1 | Complete |
| REC-05 | Phase 1 | Complete |
| REC-06 | Phase 1 | Complete |
| TRX-01 | Phase 2 | Complete |
| TRX-02 | Phase 2 | Complete |
| TRX-03 | Phase 2 | Complete |
| POL-01 | Phase 2 | Complete |
| POL-02 | Phase 2 | Complete |
| POL-03 | Phase 2 | Complete |
| POL-04 | Phase 2 | Complete |
| POL-05 | Phase 3 | Complete |
| OUT-01 | Phase 2 | Complete |
| OUT-02 | Phase 2 | Complete |
| OUT-03 | Phase 2 | Complete |
| LRN-01 | Phase 3 | Complete |
| LRN-02 | Phase 3 | Complete |
| LRN-03 | Phase 3 | Complete |
| LRN-04 | Phase 3 | Complete |
| CFG-01 | Phase 1 | Complete |
| CFG-02 | Phase 1 | Complete |
| CFG-03 | Phase 3 | Complete |
| CFG-04 | Phase 4 | Complete |
| CFG-05 | Phase 3 | Complete |
| PLT-01 | Phase 1 | Complete |
| PLT-02 | Phase 1 | Complete |
| PLT-03 | Phase 4 | Complete |
| HST-01 | Phase 3 | Complete |
| HST-02 | Phase 3 | Complete |
| HST-03 | Phase 3 | Complete |

**Coverage:**
- v1 requirements: 30 total
- Mapped to phases: 30
- Unmapped: 0

---
*Requirements defined: 2025-01-29*
*Last updated: 2026-01-29 after roadmap creation*
