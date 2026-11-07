//! The `InputEngine` trait and the types engines return.

use crate::key::KeyEvent;
use std::ops::Range;

/// Text being composed but not yet committed (shown underlined by the app).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Preedit {
    pub text: String,
    /// Cursor position as a `char` index into `text`.
    pub cursor: usize,
    /// Optional highlighted `char` range (e.g. the active clause in Japanese).
    pub highlight: Option<Range<usize>>,
}

impl Preedit {
    /// A preedit with the cursor at the end and no highlight.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self {
            text,
            cursor,
            highlight: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Conversion candidates shown in a popup (kanji, hanja, hanzi, ...).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CandidateList {
    pub items: Vec<String>,
    pub selected: usize,
    /// Optional labels, aligned with items (e.g. Korean Hanja meanings).
    pub annotations: Vec<String>,
    /// The text being converted, for engines whose preedit shows something
    /// else (the Japanese preedit is the chosen candidate, not the reading).
    /// Empty means the preedit itself is the reading, as in Korean Hanja.
    pub reading: String,
}

impl CandidateList {
    pub const PAGE_SIZE: usize = 9;
}

/// What an engine produced in response to one key event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineOutput {
    /// Text to insert into the application now.
    pub commit: String,
    /// Current composition (replaces any previous preedit).
    pub preedit: Preedit,
    /// Candidate popup contents, if any.
    pub candidates: Option<CandidateList>,
    /// False means the frontend must forward the key to the application.
    pub handled: bool,
}

/// A language engine. Implementations must not depend on any OS or UI crate.
pub trait InputEngine {
    /// Handle one key press.
    fn process_key(&mut self, key: KeyEvent) -> EngineOutput;

    /// Drop the composition state without committing (e.g. on focus loss).
    fn reset(&mut self);

    /// Finish the composition and return the text to commit.
    fn flush(&mut self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preedit_cursor_counts_chars_not_bytes() {
        let p = Preedit::new("한글");
        assert_eq!(p.cursor, 2);
        assert!(!p.is_empty());
    }

    #[test]
    fn default_output_is_unhandled_and_empty() {
        let o = EngineOutput::default();
        assert!(!o.handled);
        assert!(o.commit.is_empty());
        assert!(o.preedit.is_empty());
        assert!(o.candidates.is_none());
    }
}
