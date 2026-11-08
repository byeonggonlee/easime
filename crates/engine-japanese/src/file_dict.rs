//! A compact, read-only binary dictionary (`.bin`) for fast startup.
//!
//! Loading a million entries into a `HashMap<String, Vec<String>>` takes a
//! lot of time and memory (one allocation per key and per candidate).
//! This format keeps everything in a few flat byte sections instead, and
//! a lookup is a binary search over the sorted readings.
//!
//! # Layout (all integers little endian)
//!
//! ```text
//! offset  size  field
//!      0     8  magic            b"EASIMEDI"
//!      8     4  version          1
//!     12     4  n                number of readings
//!     16     4  meta_len         bytes of the free-form UTF-8 meta text
//!     20     4  key_blob_len
//!     24     4  cand_blob_len
//!     28     4  flags            0 (reserved)
//!     32        meta             meta_len bytes (attribution text)
//!               key_offsets      (n + 1) * u32, into the key blob
//!               cand_offsets     (n + 1) * u32, into the candidate blob
//!               key_blob         readings, UTF-8, sorted bytewise, unique
//!               cand_blob        one block per reading
//! ```
//!
//! A candidate block is `varint count`, then `count` times
//! `varint byte_length, UTF-8 bytes`. Varints are LEB128 (at most 5 bytes).
//! Reading `i` is `key_blob[key_offsets[i]..key_offsets[i + 1]]` and its
//! block is `cand_blob[cand_offsets[i]..cand_offsets[i + 1]]`.
//!
//! The whole file is validated when it is opened (section sizes, offsets,
//! sorting, UTF-8, block structure), so a corrupt or truncated file is
//! rejected with an error and lookups afterwards cannot go out of bounds.

use std::fmt;
use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::dictionary::Dictionary;

/// File signature.
pub const MAGIC: [u8; 8] = *b"EASIMEDI";
/// The only format version this code reads and writes.
pub const VERSION: u32 = 1;
const HEADER_LEN: usize = 32;

/// Why a dictionary could not be read or written.
#[derive(Debug)]
pub enum DictionaryError {
    Io(io::Error),
    /// The file does not start with the easime dictionary signature.
    BadMagic,
    /// A version this code does not understand.
    UnsupportedVersion(u32),
    /// The file is shorter than its header says.
    Truncated,
    /// The file has the right size but its content is inconsistent.
    Corrupt(String),
    /// The input given to the writer is not usable.
    InvalidInput(String),
}

impl fmt::Display for DictionaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::BadMagic => write!(f, "not an easime dictionary (bad signature)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported dictionary version {v}"),
            Self::Truncated => write!(f, "dictionary file is truncated"),
            Self::Corrupt(why) => write!(f, "corrupt dictionary: {why}"),
            Self::InvalidInput(why) => write!(f, "invalid dictionary input: {why}"),
        }
    }
}

impl std::error::Error for DictionaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for DictionaryError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

fn corrupt(why: &str) -> DictionaryError {
    DictionaryError::Corrupt(why.to_string())
}

fn invalid(why: impl Into<String>) -> DictionaryError {
    DictionaryError::InvalidInput(why.into())
}

fn push_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value & 0x7f) as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Decode a varint at `*pos`, advancing it. `None` if it is cut off or
/// longer than 5 bytes or does not fit a `u32`.
fn read_varint(data: &[u8], pos: &mut usize) -> Option<u32> {
    let mut value: u64 = 0;
    for shift in 0..5 {
        let byte = *data.get(*pos)?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << (7 * shift);
        if byte & 0x80 == 0 {
            return u32::try_from(value).ok();
        }
    }
    None
}

fn read_u32(data: &[u8], pos: usize) -> Option<u32> {
    let bytes = data.get(pos..pos.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn to_u32(len: usize, what: &str) -> Result<u32, DictionaryError> {
    u32::try_from(len).map_err(|_| invalid(format!("{what} does not fit in 32 bits")))
}

/// Encode a dictionary file.
///
/// `entries` must be sorted by reading (bytewise, strictly ascending, so
/// no repeats), every reading non-empty and every candidate list non-empty
/// with non-empty candidates. `meta` is free-form attribution text.
pub fn encode<'a, I>(meta: &str, entries: I) -> Result<Vec<u8>, DictionaryError>
where
    I: IntoIterator<Item = (&'a str, &'a [String])>,
{
    let mut key_offsets: Vec<u32> = vec![0];
    let mut cand_offsets: Vec<u32> = vec![0];
    let mut key_blob: Vec<u8> = Vec::new();
    let mut cand_blob: Vec<u8> = Vec::new();
    let mut previous: Option<&str> = None;

    for (reading, candidates) in entries {
        if reading.is_empty() {
            return Err(invalid("empty reading"));
        }
        if previous.is_some_and(|p| p.as_bytes() >= reading.as_bytes()) {
            return Err(invalid(format!(
                "readings are not strictly ascending at {reading:?}"
            )));
        }
        previous = Some(reading);
        if candidates.is_empty() {
            return Err(invalid(format!("no candidates for {reading:?}")));
        }
        key_blob.extend_from_slice(reading.as_bytes());
        key_offsets.push(to_u32(key_blob.len(), "key blob")?);
        push_varint(&mut cand_blob, to_u32(candidates.len(), "candidate count")?);
        for candidate in candidates {
            if candidate.is_empty() {
                return Err(invalid(format!("empty candidate for {reading:?}")));
            }
            push_varint(&mut cand_blob, to_u32(candidate.len(), "candidate")?);
            cand_blob.extend_from_slice(candidate.as_bytes());
        }
        cand_offsets.push(to_u32(cand_blob.len(), "candidate blob")?);
    }

    let n = key_offsets.len() - 1;
    let mut out = Vec::with_capacity(
        HEADER_LEN + meta.len() + 8 * (n + 1) + key_blob.len() + cand_blob.len(),
    );
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&to_u32(n, "reading count")?.to_le_bytes());
    out.extend_from_slice(&to_u32(meta.len(), "meta text")?.to_le_bytes());
    out.extend_from_slice(&to_u32(key_blob.len(), "key blob")?.to_le_bytes());
    out.extend_from_slice(&to_u32(cand_blob.len(), "candidate blob")?.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(meta.as_bytes());
    for offset in key_offsets.iter().chain(&cand_offsets) {
        out.extend_from_slice(&offset.to_le_bytes());
    }
    out.extend_from_slice(&key_blob);
    out.extend_from_slice(&cand_blob);
    Ok(out)
}

/// The bytes of a dictionary file: read into memory, or mapped from the file.
#[derive(Clone)]
enum Bytes {
    Owned(Vec<u8>),
    Mapped(Arc<memmap2::Mmap>),
}

impl std::ops::Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(data) => data,
            Self::Mapped(map) => map,
        }
    }
}

/// A validated binary dictionary, read into memory or mapped from its file.
#[derive(Clone)]
pub struct FileDictionary {
    data: Bytes,
    count: usize,
    meta: (usize, usize),
    key_offsets: usize,
    cand_offsets: usize,
    key_blob: usize,
    cand_blob: usize,
}

impl fmt::Debug for FileDictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDictionary")
            .field("readings", &self.count)
            .field("bytes", &self.data.len())
            .finish()
    }
}

impl FileDictionary {
    /// Read and validate the file at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DictionaryError> {
        Self::from_bytes(std::fs::read(path)?)
    }

    /// Map the file at `path` into memory and validate it. Processes that map
    /// the same file share its pages, where reading it would give each its own
    /// copy (the Windows text service is loaded into every application).
    ///
    /// The file must not be shortened while it is mapped: Windows refuses
    /// that, other systems end the process. Replacing it (a new file renamed
    /// over it, as installers and packages do) is fine.
    pub fn map(path: impl AsRef<Path>) -> Result<Self, DictionaryError> {
        let file = std::fs::File::open(path)?;
        // SAFETY: a read-only mapping. Another program changing the file in
        // place can only make lookups (all bounds-checked) find wrong words;
        // shortening it is what the documentation above rules out.
        let map = unsafe { memmap2::Mmap::map(&file) }?;
        Self::from_data(Bytes::Mapped(Arc::new(map)))
    }

    /// Validate `data` as a dictionary file and take ownership of it.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, DictionaryError> {
        Self::from_data(Bytes::Owned(data))
    }

    fn from_data(data: Bytes) -> Result<Self, DictionaryError> {
        if data.len() < HEADER_LEN {
            // A file that is not even a header can still be recognised as
            // foreign rather than truncated.
            return Err(if data.starts_with(&MAGIC) || MAGIC.starts_with(&data) {
                DictionaryError::Truncated
            } else {
                DictionaryError::BadMagic
            });
        }
        if data[..8] != MAGIC {
            return Err(DictionaryError::BadMagic);
        }
        let header = |i: usize| read_u32(&data, 8 + 4 * i).ok_or(DictionaryError::Truncated);
        let version = header(0)?;
        if version != VERSION {
            return Err(DictionaryError::UnsupportedVersion(version));
        }
        let count = header(1)? as usize;
        let meta_len = header(2)? as usize;
        let key_blob_len = header(3)? as usize;
        let cand_blob_len = header(4)? as usize;
        if header(5)? != 0 {
            return Err(corrupt("unknown flags"));
        }

        // Section positions, with checked arithmetic (32-bit targets too).
        let overflow = || corrupt("section sizes overflow");
        let table_len = count
            .checked_add(1)
            .and_then(|c| c.checked_mul(4))
            .ok_or_else(overflow)?;
        let meta_start = HEADER_LEN;
        let key_offsets = meta_start.checked_add(meta_len).ok_or_else(overflow)?;
        let cand_offsets = key_offsets.checked_add(table_len).ok_or_else(overflow)?;
        let key_blob = cand_offsets.checked_add(table_len).ok_or_else(overflow)?;
        let cand_blob = key_blob.checked_add(key_blob_len).ok_or_else(overflow)?;
        let end = cand_blob.checked_add(cand_blob_len).ok_or_else(overflow)?;
        if data.len() < end {
            return Err(DictionaryError::Truncated);
        }
        if data.len() > end {
            return Err(corrupt("trailing bytes after the last section"));
        }

        let dict = Self {
            count,
            meta: (meta_start, meta_len),
            key_offsets,
            cand_offsets,
            key_blob,
            cand_blob,
            data,
        };
        dict.validate(key_blob_len, cand_blob_len)?;
        Ok(dict)
    }

    fn validate(&self, key_blob_len: usize, cand_blob_len: usize) -> Result<(), DictionaryError> {
        let data = &self.data[..];
        std::str::from_utf8(&data[self.meta.0..self.meta.0 + self.meta.1])
            .map_err(|_| corrupt("meta text is not UTF-8"))?;
        let keys = std::str::from_utf8(&data[self.key_blob..self.key_blob + key_blob_len])
            .map_err(|_| corrupt("reading blob is not UTF-8"))?;

        let mut key_start = 0usize;
        let mut cand_start = 0usize;
        if self.table(self.key_offsets, 0) != Some(0) || self.table(self.cand_offsets, 0) != Some(0)
        {
            return Err(corrupt("offset tables do not start at 0"));
        }
        let mut previous: Option<&str> = None;
        for i in 0..self.count {
            let key_end = self
                .table(self.key_offsets, i + 1)
                .ok_or(DictionaryError::Truncated)? as usize;
            let cand_end = self
                .table(self.cand_offsets, i + 1)
                .ok_or(DictionaryError::Truncated)? as usize;
            let reading = keys
                .get(key_start..key_end)
                .ok_or_else(|| corrupt("reading offsets out of order or inside a character"))?;
            if reading.is_empty() {
                return Err(corrupt("empty reading"));
            }
            if previous.is_some_and(|p| p.as_bytes() >= reading.as_bytes()) {
                return Err(corrupt("readings are not strictly ascending"));
            }
            previous = Some(reading);
            let block = data[self.cand_blob..self.cand_blob + cand_blob_len]
                .get(cand_start..cand_end)
                .ok_or_else(|| corrupt("candidate offsets out of order or out of range"))?;
            if Self::walk_block(block, |_| {}).is_none() {
                return Err(corrupt("malformed candidate block"));
            }
            key_start = key_end;
            cand_start = cand_end;
        }
        if key_start != key_blob_len || cand_start != cand_blob_len {
            return Err(corrupt("offset tables do not cover the blobs"));
        }
        Ok(())
    }

    /// Entry `index` of the u32 table starting at `table`.
    fn table(&self, table: usize, index: usize) -> Option<u32> {
        read_u32(&self.data, table.checked_add(index.checked_mul(4)?)?)
    }

    /// Split a candidate block into its strings. `None` if the block is
    /// malformed (it must be consumed exactly, with non-empty UTF-8 items).
    fn parse_block(block: &[u8]) -> Option<Vec<&str>> {
        let mut out = Vec::new();
        Self::walk_block(block, |candidate| out.push(candidate))?;
        Some(out)
    }

    /// Call `f` with each candidate of a block. `None` if the block is
    /// malformed; nothing is allocated here.
    fn walk_block<'a>(block: &'a [u8], mut f: impl FnMut(&'a str)) -> Option<()> {
        let mut pos = 0;
        let count = read_varint(block, &mut pos)?;
        if count == 0 {
            return None;
        }
        for _ in 0..count {
            let len = read_varint(block, &mut pos)? as usize;
            if len == 0 {
                return None;
            }
            let bytes = block.get(pos..pos.checked_add(len)?)?;
            pos += len;
            f(std::str::from_utf8(bytes).ok()?);
        }
        (pos == block.len()).then_some(())
    }

    fn key(&self, index: usize) -> Option<&[u8]> {
        let start = self.table(self.key_offsets, index)? as usize;
        let end = self.table(self.key_offsets, index + 1)? as usize;
        self.data
            .get(self.key_blob.checked_add(start)?..self.key_blob.checked_add(end)?)
    }

    fn block(&self, index: usize) -> Option<&[u8]> {
        let start = self.table(self.cand_offsets, index)? as usize;
        let end = self.table(self.cand_offsets, index + 1)? as usize;
        self.data
            .get(self.cand_blob.checked_add(start)?..self.cand_blob.checked_add(end)?)
    }

    /// Candidates for exactly `reading`, borrowed from the file data.
    pub fn candidates(&self, reading: &str) -> Option<Vec<&str>> {
        let target = reading.as_bytes();
        let (mut low, mut high) = (0usize, self.count);
        while low < high {
            let mid = low + (high - low) / 2;
            match self.key(mid)?.cmp(target) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Self::parse_block(self.block(mid)?),
            }
        }
        None
    }

    /// The `index`-th reading in sorted order, for iterating a dictionary.
    pub fn reading_at(&self, index: usize) -> Option<&str> {
        std::str::from_utf8(self.key(index)?).ok()
    }

    /// Number of readings.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Whether the file is mapped ([`FileDictionary::map`]) rather than read.
    pub fn is_mapped(&self) -> bool {
        matches!(self.data, Bytes::Mapped(_))
    }

    /// Size of the file data, in bytes.
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    /// The free-form attribution text stored in the file.
    pub fn meta(&self) -> &str {
        // Validated as UTF-8 when the file was opened.
        self.data
            .get(self.meta.0..self.meta.0 + self.meta.1)
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }
}

impl Dictionary for FileDictionary {
    fn lookup(&self, reading: &str) -> Vec<String> {
        self.candidates(reading)
            .map(|c| c.into_iter().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<(String, Vec<String>)> {
        let mut e = vec![
            ("にほん", vec!["日本", "二本"]),
            ("にほんご", vec!["日本語"]),
            ("ひと", vec!["人", "ヒト", "一"]),
            ("a", vec!["ａ"]),
            ("ー", vec!["長音"]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
        .collect::<Vec<(String, Vec<String>)>>();
        e.sort();
        e
    }

    fn encode_entries(e: &[(String, Vec<String>)]) -> Vec<u8> {
        encode("meta ✓", e.iter().map(|(k, v)| (k.as_str(), v.as_slice()))).unwrap()
    }

    fn sample() -> Vec<u8> {
        encode_entries(&entries())
    }

    #[test]
    fn round_trips_every_entry() {
        let e = entries();
        let d = FileDictionary::from_bytes(encode_entries(&e)).unwrap();
        assert_eq!(d.len(), e.len());
        assert_eq!(d.meta(), "meta ✓");
        for (i, (reading, candidates)) in e.iter().enumerate() {
            assert_eq!(&d.lookup(reading), candidates, "{reading}");
            assert_eq!(d.reading_at(i), Some(reading.as_str()));
        }
        assert_eq!(d.reading_at(e.len()), None);
    }

    #[test]
    fn candidate_order_is_kept() {
        let d = FileDictionary::from_bytes(sample()).unwrap();
        assert_eq!(d.lookup("ひと"), ["人", "ヒト", "一"]);
        assert_eq!(d.candidates("ひと").unwrap(), ["人", "ヒト", "一"]);
    }

    #[test]
    fn missing_keys_are_empty() {
        let d = FileDictionary::from_bytes(sample()).unwrap();
        assert!(d.lookup("ふめい").is_empty());
        assert!(d.lookup("").is_empty());
        // A prefix or an extension of a key is not the key.
        assert!(d.lookup("に").is_empty());
        assert!(d.lookup("にほんごご").is_empty());
        assert!(d.candidates("zzz").is_none());
    }

    #[test]
    fn empty_dictionary_is_valid() {
        let bytes = encode("", std::iter::empty()).unwrap();
        let d = FileDictionary::from_bytes(bytes).unwrap();
        assert!(d.is_empty());
        assert!(d.lookup("にほん").is_empty());
    }

    #[test]
    fn long_candidates_use_multi_byte_lengths() {
        let long = "漢".repeat(500); // 1500 bytes
        let many: Vec<String> = (0..300).map(|i| format!("候補{i}")).collect();
        let e = vec![
            ("かん".to_string(), vec![long.clone()]),
            ("たくさん".to_string(), many.clone()),
        ];
        let d = FileDictionary::from_bytes(encode_entries(&e)).unwrap();
        assert_eq!(d.lookup("かん"), [long]);
        assert_eq!(d.lookup("たくさん"), many);
    }

    #[test]
    fn many_entries_binary_search_finds_each() {
        let e: Vec<(String, Vec<String>)> = (0..2000)
            .map(|i| (format!("{i:05}"), vec![format!("v{i}")]))
            .collect();
        let d = FileDictionary::from_bytes(encode_entries(&e)).unwrap();
        for i in (0..2000).step_by(7) {
            assert_eq!(d.lookup(&format!("{i:05}")), [format!("v{i}")]);
        }
        assert!(d.lookup("02000").is_empty());
        assert!(d.lookup("00000x").is_empty());
    }

    #[test]
    fn open_reads_a_file() {
        let path =
            std::env::temp_dir().join(format!("easime-file-dict-{}.bin", std::process::id()));
        std::fs::write(&path, sample()).unwrap();
        let d = FileDictionary::open(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(d.lookup("にほん"), ["日本", "二本"]);
        assert!(matches!(
            FileDictionary::open(&path),
            Err(DictionaryError::Io(_))
        ));
    }

    #[test]
    fn a_mapped_file_finds_the_same_words_as_a_read_one() {
        let path =
            std::env::temp_dir().join(format!("easime-file-dict-map-{}.bin", std::process::id()));
        std::fs::write(&path, sample()).unwrap();
        {
            let mapped = FileDictionary::map(&path).unwrap();
            let read = FileDictionary::open(&path).unwrap();
            assert!(mapped.is_mapped() && !read.is_mapped());
            assert_eq!(mapped.len(), read.len());
            assert_eq!(mapped.byte_len(), read.byte_len());
            assert_eq!(mapped.meta(), read.meta());
            for index in 0..read.len() {
                let reading = read.reading_at(index).unwrap();
                assert_eq!(mapped.lookup(reading), read.lookup(reading), "{reading}");
            }
            assert!(mapped.lookup("ない").is_empty());
            // A clone shares the mapping.
            let copy = mapped.clone();
            drop(mapped);
            assert_eq!(copy.lookup("にほん"), ["日本", "二本"]);
        }
        // Windows does not delete a file that is still mapped: the dictionaries
        // are gone by now.
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            FileDictionary::map(&path),
            Err(DictionaryError::Io(_))
        ));
    }

    #[test]
    fn a_mapped_file_is_checked_like_a_read_one() {
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let foreign = dir.join(format!("easime-file-dict-foreign-{pid}.bin"));
        std::fs::write(&foreign, b"hello, this is not a dictionary at all").unwrap();
        assert!(matches!(
            FileDictionary::map(&foreign),
            Err(DictionaryError::BadMagic)
        ));
        std::fs::remove_file(&foreign).unwrap();

        let empty = dir.join(format!("easime-file-dict-empty-{pid}.bin"));
        std::fs::write(&empty, b"").unwrap();
        assert!(
            FileDictionary::map(&empty).is_err(),
            "an empty file is refused"
        );
        std::fs::remove_file(&empty).unwrap();

        let truncated = dir.join(format!("easime-file-dict-short-{pid}.bin"));
        let mut bytes = sample();
        bytes.truncate(bytes.len() - 3);
        std::fs::write(&truncated, bytes).unwrap();
        assert!(matches!(
            FileDictionary::map(&truncated),
            Err(DictionaryError::Truncated)
        ));
        std::fs::remove_file(&truncated).unwrap();
    }

    #[test]
    fn writer_rejects_bad_input() {
        let one = vec!["x".to_string()];
        let none: Vec<String> = vec![];
        let e = |entries: Vec<(&str, &[String])>| encode("", entries);
        assert!(matches!(
            e(vec![("b", &one), ("a", &one)]),
            Err(DictionaryError::InvalidInput(_))
        ));
        assert!(e(vec![("a", &one), ("a", &one)]).is_err());
        assert!(e(vec![("", &one)]).is_err());
        assert!(e(vec![("a", &none)]).is_err());
        assert!(e(vec![("a", &["".to_string()])]).is_err());
    }

    #[test]
    fn rejects_foreign_and_tiny_files() {
        assert!(matches!(
            FileDictionary::from_bytes(Vec::new()),
            Err(DictionaryError::Truncated)
        ));
        assert!(matches!(
            FileDictionary::from_bytes(b"EASI".to_vec()),
            Err(DictionaryError::Truncated)
        ));
        assert!(matches!(
            FileDictionary::from_bytes(b"hello".to_vec()),
            Err(DictionaryError::BadMagic)
        ));
        let mut bytes = sample();
        bytes[0] = b'X';
        assert!(matches!(
            FileDictionary::from_bytes(bytes),
            Err(DictionaryError::BadMagic)
        ));
        let text = "にほん\t日本\n".repeat(10);
        assert!(matches!(
            FileDictionary::from_bytes(text.into_bytes()),
            Err(DictionaryError::BadMagic)
        ));
    }

    #[test]
    fn rejects_other_versions_and_flags() {
        let mut bytes = sample();
        bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
        assert!(matches!(
            FileDictionary::from_bytes(bytes),
            Err(DictionaryError::UnsupportedVersion(2))
        ));
        let mut bytes = sample();
        bytes[28] = 1;
        assert!(matches!(
            FileDictionary::from_bytes(bytes),
            Err(DictionaryError::Corrupt(_))
        ));
    }

    #[test]
    fn every_truncation_is_an_error() {
        let bytes = sample();
        for len in 0..bytes.len() {
            assert!(
                FileDictionary::from_bytes(bytes[..len].to_vec()).is_err(),
                "length {len} was accepted"
            );
        }
    }

    #[test]
    fn trailing_garbage_is_an_error() {
        let mut bytes = sample();
        bytes.push(0);
        assert!(matches!(
            FileDictionary::from_bytes(bytes),
            Err(DictionaryError::Corrupt(_))
        ));
    }

    #[test]
    fn huge_counts_in_the_header_do_not_allocate_or_panic() {
        for field in [12, 16, 20, 24] {
            let mut bytes = sample();
            bytes[field..field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(FileDictionary::from_bytes(bytes).is_err(), "field {field}");
        }
    }

    #[test]
    fn flipping_any_single_byte_never_panics() {
        // Corruption may go unnoticed (e.g. inside a candidate string) but
        // must never panic, and lookups on whatever was accepted must work.
        let bytes = sample();
        for i in 0..bytes.len() {
            for flip in [0x01u8, 0x80, 0xff] {
                let mut copy = bytes.clone();
                copy[i] ^= flip;
                if let Ok(d) = FileDictionary::from_bytes(copy) {
                    for key in ["にほん", "ひと", "a", "ー", "zz"] {
                        let _ = d.lookup(key);
                    }
                }
            }
        }
    }

    #[test]
    fn detects_unsorted_readings() {
        let one = ["x".to_string()];
        let mut bytes = encode("", vec![("b", &one[..]), ("c", &one[..])]).unwrap();
        // key blob = header + two tables of 3 u32 each; turn "b" into "d".
        let key_blob = HEADER_LEN + 2 * 12;
        assert_eq!(bytes[key_blob], b'b');
        bytes[key_blob] = b'd';
        assert!(matches!(
            FileDictionary::from_bytes(bytes),
            Err(DictionaryError::Corrupt(_))
        ));
        // Equal readings are rejected too.
        let mut bytes = encode("", vec![("b", &one[..]), ("c", &one[..])]).unwrap();
        bytes[key_blob + 1] = b'b';
        assert!(FileDictionary::from_bytes(bytes).is_err());
    }

    #[test]
    fn detects_offsets_inside_a_character() {
        let mut bytes = sample();
        let meta_len = "meta ✓".len();
        // key_offsets[2] sits at 32 + meta_len + 8; bump it by one byte so it
        // splits a three byte character or misorders offsets.
        let pos = HEADER_LEN + meta_len + 8;
        let value = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) + 1;
        bytes[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
        assert!(FileDictionary::from_bytes(bytes).is_err());
    }

    #[test]
    fn varints_round_trip() {
        for value in [0u32, 1, 127, 128, 300, 16_383, 16_384, u32::MAX] {
            let mut buf = Vec::new();
            push_varint(&mut buf, value);
            let mut pos = 0;
            assert_eq!(read_varint(&buf, &mut pos), Some(value));
            assert_eq!(pos, buf.len());
        }
        // Too long and cut off.
        assert_eq!(read_varint(&[0x80; 6], &mut 0), None);
        assert_eq!(read_varint(&[0x80], &mut 0), None);
        assert_eq!(read_varint(&[0xff, 0xff, 0xff, 0xff, 0x7f], &mut 0), None);
    }

    #[test]
    fn implements_the_dictionary_trait_object() {
        let d: Box<dyn Dictionary> = Box::new(FileDictionary::from_bytes(sample()).unwrap());
        assert_eq!(d.lookup("にほんご"), ["日本語"]);
    }
}
