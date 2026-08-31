// TTP - Cosmetics
//
// The seam between the licence layer and the only things a licence is
// allowed to affect: which sound plays and what the pill looks like.
//
// The rule this module exists to enforce, from docs/ttp-pro-design.md:
// **nothing here may change what TTP does.** Every feature that makes a
// transcription happen is free and uncapped. A locked install and an unlocked
// install take an identical path through a dictation; the only difference is
// which bytes reach rodio and which face the pill draws. If that ever stops
// being true, the purchase has become a paywall again and the sincerity that
// makes it work is gone.
//
// Degradation is silent and total. Licence missing, expired, offline, file
// corrupt — the user gets the default sounds and the plain pill, and no
// message. A cosmetic that nags about its own licence is worse than no
// cosmetic at all.

use serde::Serialize;

/// Whether cosmetic extras are unlocked on this machine.
///
/// Deliberately the only question this module asks the licence layer, and
/// deliberately infallible: any error, expiry or absence answers `false`.
pub fn unlocked() -> bool {
    // Development and review override.
    //
    // Not a hole: anyone who can set an environment variable on their own
    // machine can also patch the binary, so this defends nothing that was
    // defended before. What it buys is that the person who built the thing
    // can look at it — the maintainer's own trial expired on 2026-08-18, and
    // without this he would have to buy his own app to see whether the pill
    // blinks correctly.
    //
    // It is also the honest shape for what this gate is. Nothing behind it
    // affects whether TTP works; it decides which beep plays and whether a
    // face is drawn. A cosmetic flag does not warrant tamper-proofing.
    if matches!(
        std::env::var("TTP_COSMETICS").as_deref(),
        Ok("1") | Ok("true") | Ok("on")
    ) {
        return true;
    }
    crate::licensing::is_pro_or_trial_disk()
}

/// A selectable start/stop sound set.
///
/// `id` is what gets persisted in settings and must stay stable — renaming
/// one silently resets a user's choice back to the default.
#[derive(Debug, Clone, Serialize)]
pub struct SoundPack {
    pub id: &'static str,
    /// Shown in Settings. Deliberately a name, not a description.
    pub name: &'static str,
    /// One line, written in the same register as the manual.
    pub description: &'static str,
    /// True for the pack everyone has without buying anything.
    pub free: bool,
}

/// The pack that plays when nothing else has been chosen, or when cosmetics
/// are locked. Its id is never absent from `SOUND_PACKS`.
pub const DEFAULT_PACK_ID: &str = "default";

pub const SOUND_PACKS: &[SoundPack] = &[
    SoundPack {
        id: DEFAULT_PACK_ID,
        name: "House",
        description: "The two polite tones TTP was born with.",
        free: true,
    },
    SoundPack {
        id: "radio",
        name: "Radio",
        description: "A squelch of static, as though someone somewhere received you.",
        free: false,
    },
    SoundPack {
        id: "arcade",
        name: "Arcade",
        description: "Two square-wave blips. Your words are worth points.",
        free: false,
    },
    SoundPack {
        id: "submarine",
        name: "Submarine",
        description: "A sonar ping into the quiet. Something down there heard it.",
        free: false,
    },
    SoundPack {
        id: "typewriter",
        name: "Typewriter",
        description: "A key, and later a small bell. Nothing is actually typed.",
        free: false,
    },
    SoundPack {
        id: "bubble",
        name: "Bubble",
        description: "A drop going in, and a drop coming back out.",
        free: false,
    },
];

/// Resolve the sound pack that should actually play.
///
/// Falls back to the default whenever the requested pack is unknown or is
/// locked — never errors, never tells the user. A stale id in settings (an
/// old build, a hand-edited file) simply plays the house sounds.
pub fn effective_sound_pack(requested: Option<&str>) -> &'static SoundPack {
    let default = SOUND_PACKS
        .iter()
        .find(|p| p.id == DEFAULT_PACK_ID)
        .expect("default sound pack must exist");

    let Some(id) = requested else { return default };
    let Some(pack) = SOUND_PACKS.iter().find(|p| p.id == id) else {
        return default;
    };
    if pack.free || unlocked() {
        pack
    } else {
        default
    }
}

/// Expose the catalogue to Settings. Includes locked packs on purpose: the
/// user should be able to see what the Companion sounds like before deciding,
/// and a list that hides its own contents cannot tempt anyone.
#[tauri::command]
pub fn list_sound_packs() -> Vec<SoundPack> {
    SOUND_PACKS.to_vec()
}

/// Whether the companion cosmetics are unlocked, for the Settings UI.
#[tauri::command]
pub fn cosmetics_unlocked() -> bool {
    unlocked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pack_exists_and_is_free() {
        let d = SOUND_PACKS.iter().find(|p| p.id == DEFAULT_PACK_ID);
        assert!(d.is_some());
        assert!(d.unwrap().free);
    }

    #[test]
    fn pack_ids_are_unique() {
        // A duplicate id would make `effective_sound_pack` resolve to
        // whichever came first, silently.
        let mut ids: Vec<&str> = SOUND_PACKS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate sound pack id");
    }

    #[test]
    fn unknown_pack_falls_back_to_default() {
        // A stale id from an older build, or a hand-edited settings file.
        assert_eq!(
            effective_sound_pack(Some("no-such-pack")).id,
            DEFAULT_PACK_ID
        );
    }

    #[test]
    fn no_selection_falls_back_to_default() {
        assert_eq!(effective_sound_pack(None).id, DEFAULT_PACK_ID);
    }

    #[test]
    fn the_env_override_only_answers_to_exact_values() {
        // A stray or empty TTP_COSMETICS must not unlock anything — the
        // variable is a deliberate act, not a typo.
        for v in ["", "0", "false", "yes", "TRUE"] {
            // SAFETY: single-threaded test, restored immediately.
            unsafe { std::env::set_var("TTP_COSMETICS", v) };
            let got = unlocked();
            unsafe { std::env::remove_var("TTP_COSMETICS") };
            assert!(!got, "TTP_COSMETICS={:?} should not unlock", v);
        }
    }

    #[test]
    fn the_default_is_always_available_even_locked() {
        // Whatever the licence says, the house sounds play. This is the
        // "silent degradation" guarantee: there is no state in which TTP
        // goes quiet because of a licence.
        assert_eq!(
            effective_sound_pack(Some(DEFAULT_PACK_ID)).id,
            DEFAULT_PACK_ID
        );
    }

    #[test]
    fn every_pack_has_a_name_and_a_description() {
        for p in SOUND_PACKS {
            assert!(!p.name.is_empty(), "{} has no name", p.id);
            assert!(!p.description.is_empty(), "{} has no description", p.id);
        }
    }
}
