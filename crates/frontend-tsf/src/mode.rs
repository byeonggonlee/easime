//! The input mode easime tells Windows about: whether the keyboard is "open"
//! and whether it types the native script.
//!
//! Windows keeps these two values for every thread (the TSF compartments
//! `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE` and
//! `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION`) and shows them to programs
//! that use the old input method interface (IMM32) as the open status and the
//! conversion mode of their input context. Such programs reach a text service
//! through a bridge of Windows, and that bridge finishes a composition that is
//! going on while the keyboard is closed. A text service that composes without
//! ever opening the keyboard therefore loses every composition after one key:
//! Excel's cells showed each jamo on its own (ㅎㅏㄴㄱㅡㄹ, 2026-11-10). Microsoft's
//! Korean input method and the open-source ones keep both values in step with
//! their mode, and so does easime.
//!
//! The values are plain numbers, so the mapping is tested on every OS; the
//! service writes them (`service.rs`).

use ime_core::Language;

/// `TF_CONVERSIONMODE_ALPHANUMERIC`: Latin letters, as the keys say.
pub const ALPHANUMERIC: u32 = 0x0000;
/// `TF_CONVERSIONMODE_NATIVE`: the script of the language (Hangul mode, for a
/// keyboard listed under Korean).
pub const NATIVE: u32 = 0x0001;

/// What Windows is told about the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputMode {
    /// The keyboard open/close state: open while easime composes text.
    pub open: bool,
    /// The conversion mode ([`ALPHANUMERIC`] or [`NATIVE`]).
    pub conversion: u32,
}

/// The mode of `language`: closed and alphanumeric in English (the keys go to
/// the program as they are), open and native in every language easime
/// composes. Only these two flags are used: the others (full shape, katakana,
/// roman) mean other things under a Korean keyboard, which is what easime is
/// listed as.
pub fn of(language: Language) -> InputMode {
    if language == Language::English {
        InputMode {
            open: false,
            conversion: ALPHANUMERIC,
        }
    } else {
        InputMode {
            open: true,
            conversion: NATIVE,
        }
    }
}

/// Whether the text being composed is shown the Korean way: one syllable that
/// the selection covers as an "interim character" (the block the Microsoft
/// Korean input method shows), instead of a cursor after it. `units` is the
/// length of the composition in UTF-16 units; an interim character is exactly
/// one. Programs with Korean-specific handling expect it, and the bridge for
/// IMM32 programs passes it on as the Korean composition message.
pub fn interim_character(language: Language, units: usize) -> bool {
    language == Language::Korean && units == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_closed_and_every_composing_language_is_open() {
        assert_eq!(
            of(Language::English),
            InputMode {
                open: false,
                conversion: ALPHANUMERIC
            }
        );
        for language in [
            Language::Korean,
            Language::Japanese,
            Language::ChineseSimplified,
            Language::ChineseTraditional,
        ] {
            assert_eq!(
                of(language),
                InputMode {
                    open: true,
                    conversion: NATIVE
                },
                "{language:?}"
            );
        }
    }

    #[test]
    fn only_one_korean_syllable_is_an_interim_character() {
        assert!(interim_character(Language::Korean, 1));
        assert!(!interim_character(Language::Korean, 0), "nothing composed");
        assert!(!interim_character(Language::Korean, 2));
        assert!(
            !interim_character(Language::Japanese, 1),
            "kana grow into a reading: a cursor after them"
        );
        assert!(!interim_character(Language::English, 1));
    }
}
