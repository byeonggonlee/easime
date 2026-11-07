//! Reading to candidate lookup used for kana to kanji conversion.

use std::collections::HashMap;

/// Looks up conversion candidates for a hiragana reading.
pub trait Dictionary: Send {
    /// Candidates for exactly `reading`, best first. Empty if unknown.
    fn lookup(&self, reading: &str) -> Vec<String>;
}

/// A dictionary that knows nothing (conversion then only offers kana).
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyDictionary;

impl Dictionary for EmptyDictionary {
    fn lookup(&self, _reading: &str) -> Vec<String> {
        Vec::new()
    }
}

/// An in-memory dictionary, loaded from tab separated text:
///
/// ```text
/// # comment
/// にほん<TAB>日本,二本
/// ```
///
/// The first column is the reading, the second the comma separated
/// candidates in order of preference. Repeated readings are merged.
#[derive(Debug, Default, Clone)]
pub struct MemoryDictionary {
    entries: HashMap<String, Vec<String>>,
}

impl MemoryDictionary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse dictionary text. Malformed lines are skipped; the number of
    /// skipped lines is returned alongside the dictionary.
    pub fn parse(text: &str) -> (Self, usize) {
        let mut dict = Self::default();
        let mut skipped = 0;
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let mut columns = line.splitn(2, '\t');
            let (Some(reading), Some(candidates)) = (columns.next(), columns.next()) else {
                skipped += 1;
                continue;
            };
            let reading = reading.trim();
            if reading.is_empty() {
                skipped += 1;
                continue;
            }
            let candidates: Vec<&str> = candidates
                .split(',')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .collect();
            if candidates.is_empty() {
                skipped += 1;
                continue;
            }
            for candidate in candidates {
                dict.add(reading, candidate);
            }
        }
        (dict, skipped)
    }

    /// Add one candidate for a reading (after the ones already there).
    pub fn add(&mut self, reading: &str, candidate: &str) {
        let list = self.entries.entry(reading.to_string()).or_default();
        if !list.iter().any(|c| c == candidate) {
            list.push(candidate.to_string());
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Dictionary for MemoryDictionary {
    fn lookup(&self, reading: &str) -> Vec<String> {
        self.entries.get(reading).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dictionary_knows_nothing() {
        assert!(EmptyDictionary.lookup("にほん").is_empty());
    }

    #[test]
    fn parses_entries_in_order() {
        let (d, skipped) = MemoryDictionary::parse("にほん\t日本,二本\nひと\t人\n");
        assert_eq!(skipped, 0);
        assert_eq!(d.lookup("にほん"), ["日本", "二本"]);
        assert_eq!(d.lookup("ひと"), ["人"]);
        assert!(d.lookup("ふめい").is_empty());
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let (d, skipped) = MemoryDictionary::parse("# header\n\n  \nひと\t人\n");
        assert_eq!(skipped, 0);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn counts_and_skips_malformed_lines() {
        let (d, skipped) = MemoryDictionary::parse("no tab here\n\t日本\nひと\t\nひと\t人\n");
        assert_eq!(skipped, 3);
        assert_eq!(d.lookup("ひと"), ["人"]);
    }

    #[test]
    fn merges_repeated_readings_without_duplicates() {
        let (d, _) = MemoryDictionary::parse("かみ\t神,紙\nかみ\t髪,紙\n");
        assert_eq!(d.lookup("かみ"), ["神", "紙", "髪"]);
    }

    #[test]
    fn handles_windows_line_endings() {
        let (d, skipped) = MemoryDictionary::parse("ひと\t人\r\nいぬ\t犬\r\n");
        assert_eq!(skipped, 0);
        assert_eq!(d.lookup("いぬ"), ["犬"]);
    }
}
