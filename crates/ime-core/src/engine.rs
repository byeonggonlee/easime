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
    /// Candidates per page. Every page is numbered 1 to `PAGE_SIZE` and the
    /// number keys pick from the page on screen, on every OS.
    pub const PAGE_SIZE: usize = 9;

    /// Index of the first candidate on the page that holds the selection.
    pub fn page_start(&self) -> usize {
        if self.items.is_empty() {
            return 0;
        }
        let selected = self.selected.min(self.items.len() - 1);
        selected / Self::PAGE_SIZE * Self::PAGE_SIZE
    }

    /// One past the index of the last candidate on that page.
    pub fn page_end(&self) -> usize {
        (self.page_start() + Self::PAGE_SIZE).min(self.items.len())
    }

    /// Number of the page holding the selection, counted from 1.
    pub fn page_number(&self) -> usize {
        self.page_start() / Self::PAGE_SIZE + 1
    }

    /// How many pages there are.
    pub fn page_count(&self) -> usize {
        self.items.len().div_ceil(Self::PAGE_SIZE)
    }
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

    /// Whether this key is the engine's business right now, answered without
    /// changing anything. Some frontends have to say so *before* they give the
    /// key over (Windows TSF asks "will you take this key?" first and promises
    /// the answer has no side effects).
    ///
    /// `false` is a promise: `process_key` would commit nothing, leave the
    /// composition as it is and report `handled == false`, so the application
    /// can have the key. `true` may be generous: `process_key` can still end up
    /// leaving the key to the application, after committing the finished text.
    fn wants_key(&self, key: KeyEvent) -> bool;

    /// Drop the composition state without committing (e.g. on focus loss).
    fn reset(&mut self);

    /// Finish the composition and return the text to commit.
    fn flush(&mut self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: usize, selected: usize) -> CandidateList {
        CandidateList {
            items: (0..items).map(|n| n.to_string()).collect(),
            selected,
            ..CandidateList::default()
        }
    }

    #[test]
    fn the_page_follows_the_selection() {
        let first = list(11, 0);
        assert_eq!((first.page_start(), first.page_end()), (0, 9));
        assert_eq!((first.page_number(), first.page_count()), (1, 2));
        let eighth = list(11, 8);
        assert_eq!((eighth.page_start(), eighth.page_end()), (0, 9));
        let ninth = list(11, 9);
        assert_eq!((ninth.page_start(), ninth.page_end()), (9, 11));
        assert_eq!((ninth.page_number(), ninth.page_count()), (2, 2));
    }

    #[test]
    fn a_full_last_page_and_an_empty_list() {
        let exact = list(18, 17);
        assert_eq!((exact.page_start(), exact.page_end()), (9, 18));
        assert_eq!(exact.page_count(), 2);
        let empty = list(0, 0);
        assert_eq!((empty.page_start(), empty.page_end()), (0, 0));
        assert_eq!(empty.page_count(), 0);
    }

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
