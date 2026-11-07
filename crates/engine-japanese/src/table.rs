//! The romaji to hiragana table.

/// `(romaji, hiragana)` pairs. Order does not matter.
pub const ROMAJI: &[(&str, &str)] = &[
    // Vowels
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    // K
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("kya", "きゃ"),
    ("kyi", "きぃ"),
    ("kyu", "きゅ"),
    ("kye", "きぇ"),
    ("kyo", "きょ"),
    // G
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("gya", "ぎゃ"),
    ("gyi", "ぎぃ"),
    ("gyu", "ぎゅ"),
    ("gye", "ぎぇ"),
    ("gyo", "ぎょ"),
    // S
    ("sa", "さ"),
    ("si", "し"),
    ("shi", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("sya", "しゃ"),
    ("syi", "しぃ"),
    ("syu", "しゅ"),
    ("sye", "しぇ"),
    ("syo", "しょ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("she", "しぇ"),
    ("sho", "しょ"),
    // Z / J
    ("za", "ざ"),
    ("zi", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("zya", "じゃ"),
    ("zyi", "じぃ"),
    ("zyu", "じゅ"),
    ("zye", "じぇ"),
    ("zyo", "じょ"),
    ("ja", "じゃ"),
    ("ji", "じ"),
    ("ju", "じゅ"),
    ("je", "じぇ"),
    ("jo", "じょ"),
    ("jya", "じゃ"),
    ("jyi", "じぃ"),
    ("jyu", "じゅ"),
    ("jye", "じぇ"),
    ("jyo", "じょ"),
    // T
    ("ta", "た"),
    ("ti", "ち"),
    ("chi", "ち"),
    ("tu", "つ"),
    ("tsu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("tya", "ちゃ"),
    ("tyi", "ちぃ"),
    ("tyu", "ちゅ"),
    ("tye", "ちぇ"),
    ("tyo", "ちょ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("che", "ちぇ"),
    ("cho", "ちょ"),
    ("cya", "ちゃ"),
    ("cyi", "ちぃ"),
    ("cyu", "ちゅ"),
    ("cye", "ちぇ"),
    ("cyo", "ちょ"),
    ("tsa", "つぁ"),
    ("tsi", "つぃ"),
    ("tse", "つぇ"),
    ("tso", "つぉ"),
    ("tha", "てゃ"),
    ("thi", "てぃ"),
    ("thu", "てゅ"),
    ("the", "てぇ"),
    ("tho", "てょ"),
    // D
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("dya", "ぢゃ"),
    ("dyi", "ぢぃ"),
    ("dyu", "ぢゅ"),
    ("dye", "ぢぇ"),
    ("dyo", "ぢょ"),
    ("dha", "でゃ"),
    ("dhi", "でぃ"),
    ("dhu", "でゅ"),
    ("dhe", "でぇ"),
    ("dho", "でょ"),
    // N
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("nya", "にゃ"),
    ("nyi", "にぃ"),
    ("nyu", "にゅ"),
    ("nye", "にぇ"),
    ("nyo", "にょ"),
    ("nn", "ん"),
    ("n'", "ん"),
    // H / F
    ("ha", "は"),
    ("hi", "ひ"),
    ("hu", "ふ"),
    ("fu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("hya", "ひゃ"),
    ("hyi", "ひぃ"),
    ("hyu", "ひゅ"),
    ("hye", "ひぇ"),
    ("hyo", "ひょ"),
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    ("fya", "ふゃ"),
    ("fyu", "ふゅ"),
    ("fyo", "ふょ"),
    // B
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    ("bya", "びゃ"),
    ("byi", "びぃ"),
    ("byu", "びゅ"),
    ("bye", "びぇ"),
    ("byo", "びょ"),
    // P
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    ("pya", "ぴゃ"),
    ("pyi", "ぴぃ"),
    ("pyu", "ぴゅ"),
    ("pye", "ぴぇ"),
    ("pyo", "ぴょ"),
    // M
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("mya", "みゃ"),
    ("myi", "みぃ"),
    ("myu", "みゅ"),
    ("mye", "みぇ"),
    ("myo", "みょ"),
    // Y
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    // R
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("rya", "りゃ"),
    ("ryi", "りぃ"),
    ("ryu", "りゅ"),
    ("rye", "りぇ"),
    ("ryo", "りょ"),
    // W
    ("wa", "わ"),
    ("wi", "うぃ"),
    ("wu", "う"),
    ("we", "うぇ"),
    ("wo", "を"),
    ("wha", "うぁ"),
    ("whi", "うぃ"),
    ("whu", "う"),
    ("whe", "うぇ"),
    ("who", "うぉ"),
    // V / Q
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("qa", "くぁ"),
    ("qi", "くぃ"),
    ("qu", "く"),
    ("qe", "くぇ"),
    ("qo", "くぉ"),
    // Small kana (x.. and l..)
    ("xa", "ぁ"),
    ("xi", "ぃ"),
    ("xu", "ぅ"),
    ("xe", "ぇ"),
    ("xo", "ぉ"),
    ("la", "ぁ"),
    ("li", "ぃ"),
    ("lu", "ぅ"),
    ("le", "ぇ"),
    ("lo", "ぉ"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
    ("xwa", "ゎ"),
    ("lwa", "ゎ"),
    ("xtu", "っ"),
    ("ltu", "っ"),
    ("xtsu", "っ"),
    ("ltsu", "っ"),
    // Punctuation
    ("-", "ー"),
    (",", "、"),
    (".", "。"),
    ("[", "「"),
    ("]", "」"),
    ("/", "・"),
    ("~", "〜"),
    ("?", "？"),
    ("!", "！"),
];

/// Hiragana for exactly `romaji`, if it is a complete entry.
pub fn lookup(romaji: &str) -> Option<&'static str> {
    ROMAJI.iter().find(|(r, _)| *r == romaji).map(|(_, k)| *k)
}

/// True if some entry starts with `prefix` and is longer than it.
pub fn has_longer(prefix: &str) -> bool {
    ROMAJI
        .iter()
        .any(|(r, _)| r.len() > prefix.len() && r.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_entries() {
        assert_eq!(lookup("ka"), Some("か"));
        assert_eq!(lookup("shi"), Some("し"));
        assert_eq!(lookup("kya"), Some("きゃ"));
        assert_eq!(lookup("zzz"), None);
    }

    #[test]
    fn detects_longer_entries() {
        assert!(has_longer("k"));
        assert!(has_longer("ky"));
        assert!(has_longer("ts"));
        assert!(!has_longer("ka"));
        assert!(!has_longer("kya"));
    }

    #[test]
    fn has_no_duplicate_keys() {
        let mut keys: Vec<&str> = ROMAJI.iter().map(|(r, _)| *r).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len());
    }

    #[test]
    fn all_values_are_hiragana_or_japanese_punctuation() {
        for (r, k) in ROMAJI {
            assert!(!k.is_empty(), "{r}");
            for c in k.chars() {
                let ok = ('\u{3041}'..='\u{3096}').contains(&c)
                    || matches!(
                        c,
                        'ー' | '、' | '。' | '「' | '」' | '・' | '〜' | '？' | '！'
                    );
                assert!(ok, "{r} -> {c}");
            }
        }
    }
}
