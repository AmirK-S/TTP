# Requirements: TTP (Talk To Paste)

**Defined:** 2025-01-29
**Core Value:** One shortcut to turn speech into text, anywhere.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Core Recording

- [ ] **REC-01**: App runs as menu bar icon (macOS) / system tray icon (Windows)
- [ ] **REC-02**: User can trigger recording via global keyboard shortcut
- [ ] **REC-03**: Push-to-talk mode: hold shortcut = record, release = stop and transcribe
- [ ] **REC-04**: Double-tap shortcut toggles persistent recording (tap again to stop)
- [ ] **REC-05**: Visual indicator shows recording state (icon change + optional overlay)
- [ ] **REC-06**: Audio captured from system default microphone

### Transcription

- [ ] **TRX-01**: Audio sent to OpenAI Whisper API for transcription
- [ ] **TRX-02**: Transcription includes proper punctuation
- [ ] **TRX-03**: Multi-language support (Whisper handles 100+ languages)

### AI Polish

- [ ] **POL-01**: Transcription processed by GPT-4o-mini for cleanup
- [ ] **POL-02**: Filler words removed (um, uh, like, you know)
- [ ] **POL-03**: Grammar corrected while preserving meaning
- [ ] **POL-04**: Self-corrections handled ("Tuesday, no wait, Wednesday" → "Wednesday")
- [ ] **POL-05**: User dictionary terms injected as context for correct spelling

### Output

- [ ] **OUT-01**: Polished text auto-pasted into active text field
- [ ] **OUT-02**: Fallback to clipboard + notification when auto-paste impossible
- [ ] **OUT-03**: Original clipboard content preserved (not overwritten permanently)

### Learning System

- [ ] **LRN-01**: System detects user corrections after auto-paste
- [ ] **LRN-02**: Corrections stored locally as dictionary entries
- [ ] **LRN-03**: Future transcriptions use learned corrections for AI polish context
- [ ] **LRN-04**: User can view/edit learned corrections in settings

### Configuration

- [ ] **CFG-01**: First-run setup prompts for OpenAI API key
- [ ] **CFG-02**: API key stored securely in system keychain/credential store
- [ ] **CFG-03**: Settings UI accessible from menu bar/tray menu
- [ ] **CFG-04**: User can customize global shortcut
- [ ] **CFG-05**: User can enable/disable AI polish (raw transcription option)

### Platform Support

- [ ] **PLT-01**: Native macOS app (menu bar, accessibility permissions)
- [ ] **PLT-02**: Native Windows app (system tray, no special permissions needed)
- [ ] **PLT-03**: Consistent feature parity across platforms

### Transcription History

- [ ] **HST-01**: All transcriptions saved locally with timestamps
- [ ] **HST-02**: User can view recent transcription history
- [ ] **HST-03**: User can copy any past transcription

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
| REC-01 | Phase 1 | Pending |
| REC-02 | Phase 1 | Pending |
| REC-03 | Phase 1 | Pending |
| REC-04 | Phase 1 | Pending |
| REC-05 | Phase 1 | Pending |
| REC-06 | Phase 1 | Pending |
| TRX-01 | Phase 2 | Pending |
| TRX-02 | Phase 2 | Pending |
| TRX-03 | Phase 2 | Pending |
| POL-01 | Phase 2 | Pending |
| POL-02 | Phase 2 | Pending |
| POL-03 | Phase 2 | Pending |
| POL-04 | Phase 2 | Pending |
| POL-05 | Phase 3 | Pending |
| OUT-01 | Phase 2 | Pending |
| OUT-02 | Phase 2 | Pending |
| OUT-03 | Phase 2 | Pending |
| LRN-01 | Phase 3 | Pending |
| LRN-02 | Phase 3 | Pending |
| LRN-03 | Phase 3 | Pending |
| LRN-04 | Phase 3 | Pending |
| CFG-01 | Phase 1 | Pending |
| CFG-02 | Phase 1 | Pending |
| CFG-03 | Phase 3 | Pending |
| CFG-04 | Phase 3 | Pending |
| CFG-05 | Phase 3 | Pending |
| PLT-01 | Phase 1 | Pending |
| PLT-02 | Phase 1 | Pending |
| PLT-03 | Phase 4 | Pending |
| HST-01 | Phase 3 | Pending |
| HST-02 | Phase 3 | Pending |
| HST-03 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 30 total
- Mapped to phases: 30
- Unmapped: 0 ✓

---
*Requirements defined: 2025-01-29*
*Last updated: 2025-01-29 after initial definition*
