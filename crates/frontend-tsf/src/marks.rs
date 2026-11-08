//! How the composition is drawn: the part being composed, a reading not
//! converted yet, or the part being converted. Windows draws these as the
//! composition's underlines ("display attributes"), the way its own input
//! methods do; the text service says which part is which, and the application
//! draws the line in its own colours.
//!
//! | Mark | Language and state | Line |
//! |---|---|---|
//! | [`Mark::Composing`] | Korean being composed | thin, solid |
//! | [`Mark::Reading`] | Japanese reading, not converted | thin, dotted |
//! | [`Mark::Converting`] | the part a candidate is chosen for (Hanja, kanji) | thick, solid |

use ime_core::{Language, Preedit};
use std::ops::Range;

/// What a part of the composition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mark {
    Composing,
    Reading,
    Converting,
}

impl Mark {
    pub const ALL: [Mark; 3] = [Mark::Composing, Mark::Reading, Mark::Converting];

    /// The display attribute's id. Fixed: applications may keep it.
    pub fn guid(self) -> &'static str {
        match self {
            Mark::Composing => "{36062030-E6F1-42CA-BB9D-9E6E5193FD35}",
            Mark::Reading => "{0ED7D6EF-A42B-4626-A7E3-544A9ADA75B1}",
            Mark::Converting => "{F974B094-340E-48D8-BC6F-CE135541B7CA}",
        }
    }

    /// Its name, where Windows lists display attributes.
    pub fn description(self) -> &'static str {
        match self {
            Mark::Composing => "easime: composing",
            Mark::Reading => "easime: reading",
            Mark::Converting => "easime: converting",
        }
    }

    /// Whether the line is dotted rather than solid.
    pub fn dotted(self) -> bool {
        self == Mark::Reading
    }

    /// Whether the line is thick.
    pub fn bold(self) -> bool {
        self == Mark::Converting
    }

    /// Whether this part is being converted (rather than typed).
    pub fn converting(self) -> bool {
        self == Mark::Converting
    }
}

/// The marks over a composition in `language`, in order and covering all of
/// it, as ranges of UTF-16 units (what TSF counts in). The part the engine
/// highlights is being converted; the rest is what this language composes.
pub fn marks(language: Language, preedit: &Preedit) -> Vec<(Range<usize>, Mark)> {
    let chars = preedit.text.chars().count();
    if chars == 0 {
        return Vec::new();
    }
    let typed = match language {
        Language::Japanese => Mark::Reading,
        _ => Mark::Composing,
    };
    let highlight = preedit
        .highlight
        .clone()
        .map(|h| h.start.min(chars)..h.end.min(chars))
        .filter(|h| !h.is_empty());
    let parts = match highlight {
        None => vec![(0..chars, typed)],
        Some(h) => [
            (0..h.start, typed),
            (h.clone(), Mark::Converting),
            (h.end..chars, typed),
        ]
        .into_iter()
        .filter(|(part, _)| !part.is_empty())
        .collect(),
    };
    parts
        .into_iter()
        .map(|(part, mark)| (utf16(&preedit.text, part), mark))
        .collect()
}

/// A range of `char`s of `text` as a range of UTF-16 units.
fn utf16(text: &str, chars: Range<usize>) -> Range<usize> {
    let units = |n: usize| text.chars().take(n).map(char::len_utf16).sum::<usize>();
    units(chars.start)..units(chars.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preedit(text: &str, highlight: Option<Range<usize>>) -> Preedit {
        let mut preedit = Preedit::new(text);
        preedit.highlight = highlight;
        preedit
    }

    #[test]
    fn korean_being_composed_is_one_solid_part() {
        assert_eq!(
            marks(Language::Korean, &preedit("한", None)),
            [(0..1, Mark::Composing)]
        );
        assert!(marks(Language::Korean, &preedit("", None)).is_empty());
    }

    #[test]
    fn a_japanese_reading_is_dotted_and_its_conversion_thick() {
        assert_eq!(
            marks(Language::Japanese, &preedit("にほん", None)),
            [(0..3, Mark::Reading)]
        );
        // The engine highlights all of the converted word.
        assert_eq!(
            marks(Language::Japanese, &preedit("日本", Some(0..2))),
            [(0..2, Mark::Converting)]
        );
    }

    #[test]
    fn a_highlight_in_the_middle_splits_the_composition() {
        assert_eq!(
            marks(Language::Japanese, &preedit("きょうは", Some(1..3))),
            [
                (0..1, Mark::Reading),
                (1..3, Mark::Converting),
                (3..4, Mark::Reading)
            ]
        );
        // Out of range or empty highlights are ignored or cut to the text.
        assert_eq!(
            marks(Language::Korean, &preedit("한", Some(0..9))),
            [(0..1, Mark::Converting)]
        );
        assert_eq!(
            marks(Language::Korean, &preedit("한", Some(1..1))),
            [(0..1, Mark::Composing)]
        );
    }

    #[test]
    fn positions_are_counted_in_utf16_units() {
        // 𠀋 is outside the basic plane: two UTF-16 units.
        assert_eq!(
            marks(Language::Japanese, &preedit("𠀋か", Some(0..1))),
            [(0..2, Mark::Converting), (2..3, Mark::Reading)]
        );
    }

    #[test]
    fn every_mark_has_its_own_well_formed_id_and_line() {
        let guids: Vec<&str> = Mark::ALL.iter().map(|m| m.guid()).collect();
        for guid in &guids {
            assert_eq!(guid.len(), 38, "{guid}");
            assert!(guid.starts_with('{') && guid.ends_with('}'));
        }
        let mut unique = guids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), guids.len());
        assert!(Mark::Reading.dotted() && !Mark::Composing.dotted());
        assert!(Mark::Converting.bold() && !Mark::Reading.bold());
    }
}
