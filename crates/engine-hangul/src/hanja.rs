//! Bundled single-syllable Hanja data, with the upstream candidate order.

use ime_core::CandidateList;
use std::collections::HashMap;
use std::sync::OnceLock;

type Entry = (&'static str, &'static str);

pub(crate) fn lookup(reading: &str) -> Option<CandidateList> {
    static TABLE: OnceLock<HashMap<&str, Vec<Entry>>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table: HashMap<&str, Vec<Entry>> = HashMap::new();
        for line in include_str!("../../../data/hanja/single-syllable.txt").lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut fields = line.splitn(3, ':');
            if let (Some(key), Some(value), Some(annotation)) =
                (fields.next(), fields.next(), fields.next())
            {
                table.entry(key).or_default().push((value, annotation));
            }
        }
        table
    });
    let entries = table.get(reading)?;
    Some(CandidateList {
        items: entries.iter().map(|(value, _)| (*value).into()).collect(),
        annotations: entries.iter().map(|(_, note)| (*note).into()).collect(),
        selected: 0,
        // The Hangul preedit stays visible while converting and is the reading.
        reading: String::new(),
    })
}
