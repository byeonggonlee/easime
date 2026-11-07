//! Converts the Mozc OSS dictionary into the easime binary dictionary.
//!
//! [`mozc`] parses the text lines; [`Builder`] groups them by reading,
//! orders and trims the candidates, and encodes the result with
//! [`engine_japanese::file_dict::encode`].

pub mod mozc;

use std::collections::HashMap;
use std::io::{self, BufRead};

use engine_japanese::file_dict::{DictionaryError, encode};

pub use mozc::{Reject, parse_line};

/// Default number of candidates kept per reading.
pub const DEFAULT_MAX_CANDIDATES: usize = 30;

/// Attribution text embedded in dictionaries built from Mozc data. The
/// full notices are in `THIRD_PARTY_LICENSES.md`, shipped with the package.
pub fn mozc_notice(commit: Option<&str>) -> String {
    let commit = commit.map(|c| format!(", commit {c}")).unwrap_or_default();
    format!(
        "easime dictionary converted from the Mozc OSS dictionary \
         (https://github.com/google/mozc{commit}). \
         Mozc: Copyright 2010-2018, Google Inc., BSD-3-Clause. \
         Entries derive from IPAdic (mecab-ipadic-2.7.0): Copyright 2000, 2001, 2002, 2003 \
         Nara Institute of Science and Technology, All Rights Reserved, and from the Okinawa \
         Dictionary (public domain). Full notices: THIRD_PARTY_LICENSES.md."
    )
}

/// Counters describing a conversion run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    /// Lines read.
    pub lines: usize,
    /// Lines rejected, indexed by [`Reject::index`].
    pub rejected: [usize; mozc::REJECT_KINDS],
    /// Lines that are not valid UTF-8.
    pub invalid_utf8: usize,
    /// Accepted entries.
    pub accepted: usize,
    /// Entries dropped as repeats of a surface already listed for the reading.
    pub duplicates: usize,
    /// Entries dropped because the reading already has the maximum.
    pub over_cap: usize,
    /// Readings dropped because their only candidate was the reading itself.
    pub kana_only_readings: usize,
    /// Readings in the output.
    pub readings: usize,
    /// Candidates in the output.
    pub candidates: usize,
}

/// Collects Mozc lines and produces the sorted dictionary entries.
#[derive(Debug)]
pub struct Builder {
    max_candidates: usize,
    entries: HashMap<String, Vec<(i32, String)>>,
    stats: Stats,
}

impl Builder {
    /// `max_candidates` is the most candidates kept per reading (at least 1).
    pub fn new(max_candidates: usize) -> Self {
        Self {
            max_candidates: max_candidates.max(1),
            entries: HashMap::new(),
            stats: Stats::default(),
        }
    }

    /// Add one line of Mozc dictionary text.
    pub fn add_line(&mut self, line: &str) {
        self.stats.lines += 1;
        match parse_line(line) {
            Ok(entry) => {
                self.stats.accepted += 1;
                let list = match self.entries.get_mut(entry.reading) {
                    Some(list) => list,
                    None => self.entries.entry(entry.reading.to_string()).or_default(),
                };
                list.push((entry.cost, entry.surface.to_string()));
            }
            Err(reject) => self.stats.rejected[reject.index()] += 1,
        }
    }

    /// Add all lines from `input`. Lines that are not UTF-8 are counted
    /// and skipped; I/O errors are returned.
    pub fn add_reader(&mut self, mut input: impl BufRead) -> io::Result<()> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            if input.read_until(b'\n', &mut buf)? == 0 {
                return Ok(());
            }
            if buf.last() == Some(&b'\n') {
                buf.pop();
            }
            match std::str::from_utf8(&buf) {
                Ok(line) => self.add_line(line),
                Err(_) => {
                    self.stats.lines += 1;
                    self.stats.invalid_utf8 += 1;
                }
            }
        }
    }

    /// Order, de-duplicate and cap every reading's candidates. The result is
    /// sorted by reading (bytewise) and deterministic: candidates sort by
    /// ascending cost, ties by surface text.
    pub fn finish(mut self) -> (Vec<(String, Vec<String>)>, Stats) {
        let mut out: Vec<(String, Vec<String>)> = Vec::with_capacity(self.entries.len());
        for (reading, mut list) in self.entries {
            list.sort();
            let mut surfaces: Vec<String> = Vec::new();
            for (_, surface) in list {
                if surfaces.contains(&surface) {
                    self.stats.duplicates += 1;
                } else if surfaces.len() >= self.max_candidates {
                    self.stats.over_cap += 1;
                } else {
                    surfaces.push(surface);
                }
            }
            // The engine always offers the reading itself as the last
            // candidate, so a list that is only the reading adds nothing.
            if surfaces.len() == 1 && surfaces[0] == reading {
                self.stats.kana_only_readings += 1;
                continue;
            }
            self.stats.candidates += surfaces.len();
            out.push((reading, surfaces));
        }
        out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        self.stats.readings = out.len();
        (out, self.stats)
    }
}

/// Encode builder output as an easime dictionary file.
pub fn encode_entries(
    meta: &str,
    entries: &[(String, Vec<String>)],
) -> Result<Vec<u8>, DictionaryError> {
    encode(
        meta,
        entries
            .iter()
            .map(|(reading, candidates)| (reading.as_str(), candidates.as_slice())),
    )
}

/// Convert Mozc dictionary text to an easime dictionary file in one step.
pub fn convert(
    inputs: impl IntoIterator<Item = impl BufRead>,
    max_candidates: usize,
    meta: &str,
) -> Result<(Vec<u8>, Stats), Box<dyn std::error::Error>> {
    let mut builder = Builder::new(max_candidates);
    for input in inputs {
        builder.add_reader(input)?;
    }
    let (entries, stats) = builder.finish();
    Ok((encode_entries(meta, &entries)?, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_japanese::dictionary::Dictionary;
    use engine_japanese::file_dict::FileDictionary;

    fn build(text: &str, cap: usize) -> (Vec<(String, Vec<String>)>, Stats) {
        let mut b = Builder::new(cap);
        b.add_reader(text.as_bytes()).unwrap();
        b.finish()
    }

    #[test]
    fn groups_by_reading_and_orders_by_cost() {
        let (e, stats) = build(
            "かみ\t1\t1\t5000\t紙\nかみ\t1\t1\t4000\t神\nかみ\t1\t1\t6000\t髪\nいぬ\t1\t1\t3000\t犬\n",
            30,
        );
        assert_eq!(e[0], ("いぬ".to_string(), vec!["犬".to_string()]));
        assert_eq!(e[1].1, ["神", "紙", "髪"]);
        assert_eq!(stats.readings, 2);
        assert_eq!(stats.candidates, 4);
        assert_eq!(stats.accepted, 4);
    }

    #[test]
    fn readings_are_sorted_bytewise() {
        let (e, _) = build("ん\t1\t1\t1\tん字\nあ\t1\t1\t1\t亜\na1\t1\t1\t1\tA\n", 30);
        let keys: Vec<&str> = e.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["a1", "あ", "ん"]);
    }

    #[test]
    fn drops_duplicates_keeping_the_cheapest() {
        let (e, stats) = build(
            "かみ\t1\t1\t5000\t紙\nかみ\t9\t9\t3000\t紙\nかみ\t1\t1\t4000\t神\n",
            30,
        );
        assert_eq!(e[0].1, ["紙", "神"]);
        assert_eq!(stats.duplicates, 1);
        assert_eq!(stats.candidates, 2);
    }

    #[test]
    fn caps_candidates_per_reading() {
        let text: String = (0..10)
            .map(|i| format!("こう\t1\t1\t{}\t候{i}\n", 100 + i))
            .collect();
        let (e, stats) = build(&text, 3);
        assert_eq!(e[0].1, ["候0", "候1", "候2"]);
        assert_eq!(stats.over_cap, 7);
    }

    #[test]
    fn equal_costs_are_ordered_by_surface_for_determinism() {
        let a = build("こう\t1\t1\t5\tB\nこう\t1\t1\t5\tA\n", 30).0;
        let b = build("こう\t1\t1\t5\tA\nこう\t1\t1\t5\tB\n", 30).0;
        assert_eq!(a, b);
        assert_eq!(a[0].1, ["A", "B"]);
    }

    #[test]
    fn a_reading_that_only_maps_to_itself_is_dropped() {
        let (e, stats) = build(
            "ふむ\t1\t1\t5\tふむ\nかな\t1\t1\t5\tかな\nかな\t1\t1\t9\t仮名\n",
            30,
        );
        assert_eq!(e.len(), 1);
        // The kana candidate stays when another one exists, keeping its rank.
        assert_eq!(
            e[0],
            (
                "かな".to_string(),
                vec!["かな".to_string(), "仮名".to_string()]
            )
        );
        assert_eq!(stats.kana_only_readings, 1);
    }

    #[test]
    fn rejected_lines_are_counted_and_skipped() {
        let (e, stats) = build(
            "\n\
             bad line\n\
             あ\tx\t1\t1\t亜\n\
             あ\t1\t1\t1\t亜\tSPELLING_CORRECTION\n\
             あ\t1\t1\t1\t\n\
             あ\t1\t1\t1\t亜\n",
            30,
        );
        assert_eq!(e.len(), 1);
        assert_eq!(stats.lines, 6);
        assert_eq!(stats.rejected[Reject::Blank.index()], 1);
        assert_eq!(stats.rejected[Reject::Columns.index()], 1);
        assert_eq!(stats.rejected[Reject::Number.index()], 1);
        assert_eq!(stats.rejected[Reject::Flagged.index()], 1);
        assert_eq!(stats.rejected[Reject::Unusable.index()], 1);
        assert_eq!(stats.accepted, 1);
    }

    #[test]
    fn invalid_utf8_lines_are_skipped_not_fatal() {
        let mut b = Builder::new(30);
        let mut data = "あ\t1\t1\t1\t亜\n".as_bytes().to_vec();
        data.extend_from_slice(b"\xff\xfe\tbroken\n");
        data.extend_from_slice("い\t1\t1\t1\t位".as_bytes()); // no final newline
        b.add_reader(&data[..]).unwrap();
        let (e, stats) = b.finish();
        assert_eq!(e.len(), 2);
        assert_eq!(stats.invalid_utf8, 1);
        assert_eq!(stats.lines, 3);
    }

    #[test]
    fn output_is_identical_for_identical_input() {
        let text = "かみ\t1\t1\t5\t紙\nかみ\t1\t1\t4\t神\nいぬ\t1\t1\t3\t犬\n";
        let one = convert([text.as_bytes()], 30, "m").unwrap().0;
        let two = convert([text.as_bytes()], 30, "m").unwrap().0;
        assert_eq!(one, two);
    }

    #[test]
    fn built_file_loads_and_answers() {
        let text = "かみ\t1\t1\t5\t紙\nかみ\t1\t1\t4\t神\nいぬ\t1\t1\t3\t犬\n";
        let (bytes, stats) = convert([text.as_bytes()], 30, &mozc_notice(Some("abc"))).unwrap();
        assert_eq!(stats.readings, 2);
        let d = FileDictionary::from_bytes(bytes).unwrap();
        assert_eq!(d.lookup("かみ"), ["神", "紙"]);
        assert_eq!(d.lookup("いぬ"), ["犬"]);
        assert!(d.meta().contains("commit abc"));
        assert!(d.meta().contains("Nara Institute"));
    }

    #[test]
    fn minimum_cap_is_one() {
        let (e, _) = build("こう\t1\t1\t1\t甲\nこう\t1\t1\t2\t乙\n", 0);
        assert_eq!(e[0].1, ["甲"]);
    }
}
