//! Reading the Mozc dictionary text format.
//!
//! Each line of `dictionary00.txt` ... `dictionary09.txt` is
//!
//! ```text
//! reading<TAB>left id<TAB>right id<TAB>cost<TAB>surface[<TAB>flag]
//! ```
//!
//! The reading is hiragana (a few entries have digits or symbols), the ids
//! are part-of-speech context ids used by Mozc's connection matrix (easime
//! does not use them), the cost is a "smaller is better" integer and the
//! surface is the text to insert. The optional sixth column is a flag such
//! as `SPELLING_CORRECTION`; such entries are not ordinary conversions.

/// Longest surface (in UTF-8 bytes) kept; the real data tops out at 60.
pub const MAX_SURFACE_BYTES: usize = 255;

/// One accepted dictionary line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    pub reading: &'a str,
    pub cost: i32,
    pub surface: &'a str,
}

/// Why a line did not become an [`Entry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Blank line.
    Blank,
    /// Not 5 or 6 tab separated columns.
    Columns,
    /// An id or the cost is not an integer.
    Number,
    /// A flagged entry (spelling correction and the like).
    Flagged,
    /// Empty, control characters, or surrounding white space.
    Unusable,
}

/// Number of [`Reject`] kinds, for counters.
pub const REJECT_KINDS: usize = 5;

impl Reject {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Reject::Blank => "blank",
            Reject::Columns => "wrong column count",
            Reject::Number => "bad number",
            Reject::Flagged => "flagged (spelling correction etc.)",
            Reject::Unusable => "unusable text",
        }
    }

    pub const ALL: [Reject; REJECT_KINDS] = [
        Reject::Blank,
        Reject::Columns,
        Reject::Number,
        Reject::Flagged,
        Reject::Unusable,
    ];
}

fn usable_reading(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn usable_surface(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_SURFACE_BYTES
        && text.trim() == text
        && !text.chars().any(char::is_control)
        && !text.contains('\u{fffd}')
}

/// Parse one line (without its line break).
pub fn parse_line(line: &str) -> Result<Entry<'_>, Reject> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.trim().is_empty() {
        return Err(Reject::Blank);
    }
    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() != 5 && columns.len() != 6 {
        return Err(Reject::Columns);
    }
    let left: Result<u32, _> = columns[1].parse();
    let right: Result<u32, _> = columns[2].parse();
    let cost: Result<i32, _> = columns[3].parse();
    let (Ok(_), Ok(_), Ok(cost)) = (left, right, cost) else {
        return Err(Reject::Number);
    };
    if columns.get(5).is_some_and(|flag| !flag.is_empty()) {
        return Err(Reject::Flagged);
    }
    let (reading, surface) = (columns[0], columns[4]);
    if !usable_reading(reading) || !usable_surface(surface) {
        return Err(Reject::Unusable);
    }
    Ok(Entry {
        reading,
        cost,
        surface,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_regular_line() {
        let e = parse_line("ああると\t1851\t1851\t7129\tアアルト").unwrap();
        assert_eq!(
            e,
            Entry {
                reading: "ああると",
                cost: 7129,
                surface: "アアルト"
            }
        );
    }

    #[test]
    fn accepts_crlf_and_negative_costs() {
        let e = parse_line("あ\t1\t2\t-5\t亜\r").unwrap();
        assert_eq!(e.cost, -5);
        assert_eq!(e.surface, "亜");
    }

    #[test]
    fn blank_lines() {
        assert_eq!(parse_line(""), Err(Reject::Blank));
        assert_eq!(parse_line("  \r"), Err(Reject::Blank));
    }

    #[test]
    fn wrong_column_counts() {
        assert_eq!(parse_line("あ\t1\t2\t3"), Err(Reject::Columns));
        assert_eq!(parse_line("あ\t1\t2\t3\t亜\tx\ty"), Err(Reject::Columns));
        assert_eq!(parse_line("just text"), Err(Reject::Columns));
    }

    #[test]
    fn bad_numbers() {
        assert_eq!(parse_line("あ\tx\t2\t3\t亜"), Err(Reject::Number));
        assert_eq!(parse_line("あ\t1\t-2\t3\t亜"), Err(Reject::Number));
        assert_eq!(parse_line("あ\t1\t2\t3.5\t亜"), Err(Reject::Number));
        assert_eq!(parse_line("あ\t1\t2\t\t亜"), Err(Reject::Number));
    }

    #[test]
    fn flagged_entries_are_rejected_but_an_empty_flag_is_not() {
        assert_eq!(
            parse_line("あばんしあ\t1841\t1841\t9218\tアヴァンシア\tSPELLING_CORRECTION"),
            Err(Reject::Flagged)
        );
        assert!(parse_line("あ\t1\t2\t3\t亜\t").is_ok());
    }

    #[test]
    fn unusable_text() {
        for line in [
            "\t1\t2\t3\t亜",
            "あ\t1\t2\t3\t",
            "あ い\t1\t2\t3\t亜",
            "あ\t1\t2\t3\t \u{301}",
            "あ\t1\t2\t3\t亜 ",
            "あ\t1\t2\t3\t亜\u{7}",
            "あ\t1\t2\t3\t亜\u{fffd}",
            &format!("あ\t1\t2\t3\t{}", "漢".repeat(100)),
        ] {
            assert_eq!(parse_line(line), Err(Reject::Unusable), "{line:?}");
        }
    }

    #[test]
    fn inner_spaces_and_symbols_in_a_surface_are_fine() {
        assert!(parse_line("あ\t1\t2\t3\tA B").is_ok());
        assert!(parse_line("1こ\t1\t2\t3\t1個").is_ok());
    }

    #[test]
    fn reject_labels_and_indexes_are_distinct() {
        for (i, r) in Reject::ALL.iter().enumerate() {
            assert_eq!(r.index(), i);
            assert!(!r.label().is_empty());
        }
    }
}
