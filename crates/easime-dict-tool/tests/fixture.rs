//! End to end checks on the committed sample of real Mozc dictionary lines.

use std::path::PathBuf;
use std::process::Command;

use easime_dict_tool::{Builder, Reject, convert, encode_entries};
use engine_japanese::dictionary::Dictionary;
use engine_japanese::file_dict::FileDictionary;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mozc-sample.txt")
}

fn fixture_text() -> String {
    std::fs::read_to_string(fixture_path()).unwrap()
}

fn build(text: &str, cap: usize) -> (FileDictionary, easime_dict_tool::Stats) {
    let (bytes, stats) = convert([text.as_bytes()], cap, "test").unwrap();
    (FileDictionary::from_bytes(bytes).unwrap(), stats)
}

#[test]
fn fixture_is_small() {
    let size = std::fs::metadata(fixture_path()).unwrap().len();
    assert!(size < 20 * 1024, "fixture is {size} bytes");
}

#[test]
fn well_known_words_convert() {
    let (d, _) = build(&fixture_text(), 30);
    assert_eq!(d.lookup("にほん")[0], "日本");
    assert!(d.lookup("にほん").contains(&"二本".to_string()));
    assert_eq!(d.lookup("へんかん")[0], "変換");
    assert!(d.lookup("かんじ").contains(&"漢字".to_string()));
    assert!(d.lookup("きしゃ").contains(&"汽車".to_string()));
    assert!(d.lookup("ふめい").is_empty());
}

#[test]
fn candidates_follow_ascending_cost_without_repeats() {
    let (d, _) = build(&fixture_text(), 30);
    // Raw lines: 試し at 3500 and 4000 (twice, one with other ids), ためし at
    // 6000, plus one with surrounding spaces and one with a bad number.
    assert_eq!(d.lookup("ためし"), ["試し", "ためし"]);
}

#[test]
fn candidates_are_capped() {
    let (d, stats) = build(&fixture_text(), 5);
    assert_eq!(d.lookup("かな").len(), 5);
    assert!(stats.over_cap > 0);
}

#[test]
fn noise_lines_are_counted() {
    let (_, stats) = build(&fixture_text(), 30);
    assert_eq!(stats.rejected[Reject::Blank.index()], 1);
    assert_eq!(stats.rejected[Reject::Columns.index()], 1);
    assert_eq!(stats.rejected[Reject::Number.index()], 1);
    assert_eq!(stats.rejected[Reject::Unusable.index()], 1);
    assert!(stats.rejected[Reject::Flagged.index()] >= 1);
}

#[test]
fn spelling_corrections_are_not_offered() {
    let text = fixture_text();
    let (d, stats) = build(&text, 30);
    assert_eq!(stats.rejected[Reject::Flagged.index()], 2);
    assert_eq!(d.lookup("うるばりん"), ["ウルヴァリン", "ウルバリン"]);
    // Dropping the flagged lines by hand gives the very same dictionary.
    let without: String = text
        .lines()
        .filter(|l| !l.ends_with("SPELLING_CORRECTION"))
        .map(|l| format!("{l}\n"))
        .collect();
    let (a, _) = convert([text.as_bytes()], 30, "test").unwrap();
    let (b, _) = convert([without.as_bytes()], 30, "test").unwrap();
    assert_eq!(a, b);
    // A reading that only has flagged entries does not exist at all.
    let (d, _) = build("あ\t1\t1\t1\t亜\tSPELLING_CORRECTION\n", 30);
    assert!(d.is_empty());
}

#[test]
fn output_does_not_depend_on_line_order() {
    let text = fixture_text();
    let reversed: String = text.lines().rev().map(|l| format!("{l}\n")).collect();
    let (a, _) = convert([text.as_bytes()], 30, "test").unwrap();
    let (b, _) = convert([reversed.as_bytes()], 30, "test").unwrap();
    assert_eq!(a, b);
}

#[test]
fn splitting_the_input_over_files_changes_nothing() {
    let text = fixture_text();
    // Split on a line boundary near the middle.
    let cut = text[..text.len() / 2].rfind('\n').unwrap() + 1;
    let (one, two) = text.split_at(cut);
    let mut builder = Builder::new(30);
    builder.add_reader(one.as_bytes()).unwrap();
    builder.add_reader(two.as_bytes()).unwrap();
    let (entries, _) = builder.finish();
    let split = encode_entries("test", &entries).unwrap();
    let (whole, _) = convert([text.as_bytes()], 30, "test").unwrap();
    assert_eq!(split, whole);
}

#[test]
fn command_line_builds_and_looks_up() {
    let exe = env!("CARGO_BIN_EXE_easime-dict-tool");
    let dir = std::env::temp_dir().join(format!("easime-dict-tool-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A directory input picks up dictionary*.txt files.
    std::fs::copy(fixture_path(), dir.join("dictionary00.txt")).unwrap();
    let out = dir.join("out.bin");

    let status = Command::new(exe)
        .args(["build", "--mozc-commit", "deadbeef", "--out"])
        .arg(&out)
        .arg(&dir)
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
    assert!(!out.with_extension("tmp").exists());

    let lookup = Command::new(exe)
        .arg("lookup")
        .arg(&out)
        .args(["にほん", "ふめい"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(lookup.stdout).unwrap();
    assert!(stdout.starts_with("にほん\t日本,"), "{stdout}");
    assert!(stdout.contains("ふめい\t\n"), "{stdout}");

    let info = Command::new(exe).arg("info").arg(&out).output().unwrap();
    let stdout = String::from_utf8(info.stdout).unwrap();
    assert!(stdout.contains("commit deadbeef"), "{stdout}");

    // Bad usage fails with a message instead of panicking.
    for args in [vec![], vec!["build"], vec!["lookup"], vec!["frobnicate"]] {
        let bad = Command::new(exe).args(&args).output().unwrap();
        assert!(!bad.status.success(), "{args:?}");
        assert!(!bad.stderr.is_empty());
    }
    let missing = Command::new(exe)
        .args(["lookup", "/nonexistent/x.bin", "a"])
        .output()
        .unwrap();
    assert!(!missing.status.success());

    std::fs::remove_dir_all(&dir).ok();
}
