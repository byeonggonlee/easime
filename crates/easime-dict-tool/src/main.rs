//! Command line front end of the dictionary tool.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use easime_dict_tool::{Builder, DEFAULT_MAX_CANDIDATES, Reject, encode_entries, mozc_notice};
use engine_japanese::dictionary::{Dictionary, MemoryDictionary};
use engine_japanese::file_dict::FileDictionary;

const USAGE: &str = "\
easime-dict-tool: build and inspect easime binary dictionaries

USAGE:
  easime-dict-tool build --out FILE [--max-candidates N] [--mozc-commit SHA] INPUT...
      Convert Mozc dictionary text files to an easime binary dictionary.
      INPUT is a file, or a directory whose dictionary*.txt files are used
      (in name order).
  easime-dict-tool lookup FILE READING...
      Print the candidates of each reading.
  easime-dict-tool info FILE
      Print the size and the attribution text.
  easime-dict-tool bench FILE [--hashmap]
      Measure load time, lookup time and memory use (--hashmap: compare with
      loading the same entries into a MemoryDictionary).
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("build") => build(&args[1..]),
        Some("lookup") => lookup(&args[1..]),
        Some("info") => info(&args[1..]),
        Some("bench") => bench(&args[1..]),
        Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("missing or unknown command\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("easime-dict-tool: {message}");
            ExitCode::FAILURE
        }
    }
}

fn input_files(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(input)
                .map_err(|e| format!("{}: {e}", input.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("dictionary") && n.ends_with(".txt"))
                })
                .collect();
            found.sort();
            if found.is_empty() {
                return Err(format!("{}: no dictionary*.txt files", input.display()));
            }
            files.extend(found);
        } else {
            files.push(input.clone());
        }
    }
    Ok(files)
}

fn build(args: &[String]) -> Result<(), String> {
    let mut out: Option<PathBuf> = None;
    let mut max = DEFAULT_MAX_CANDIDATES;
    let mut commit: Option<String> = None;
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--max-candidates" => {
                max = value("--max-candidates")?
                    .parse()
                    .map_err(|_| "--max-candidates needs a number".to_string())?
            }
            "--mozc-commit" => commit = Some(value("--mozc-commit")?),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            path => inputs.push(PathBuf::from(path)),
        }
    }
    let out = out.ok_or("--out FILE is required")?;
    if inputs.is_empty() {
        return Err("no input files".into());
    }

    let started = Instant::now();
    let mut builder = Builder::new(max);
    for file in input_files(&inputs)? {
        let reader = File::open(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        builder
            .add_reader(BufReader::new(reader))
            .map_err(|e| format!("{}: {e}", file.display()))?;
    }
    let (entries, stats) = builder.finish();
    if entries.is_empty() {
        return Err("no usable entries in the input; refusing to write an empty dictionary".into());
    }
    let bytes = encode_entries(&mozc_notice(commit.as_deref()), &entries)
        .map_err(|e| format!("encoding: {e}"))?;
    // Write to a sibling temp file first so a failure never leaves a
    // half-written dictionary under the final name.
    let temp = out.with_extension("tmp");
    std::fs::write(&temp, &bytes).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, &out).map_err(|e| format!("{}: {e}", out.display()))?;

    println!(
        "{} lines, {} accepted, {} duplicate, {} over the cap of {max}, {} kana-only readings",
        stats.lines, stats.accepted, stats.duplicates, stats.over_cap, stats.kana_only_readings
    );
    for reject in Reject::ALL {
        let n = stats.rejected[reject.index()];
        if n > 0 {
            println!("  rejected {n} lines: {}", reject.label());
        }
    }
    if stats.invalid_utf8 > 0 {
        println!("  rejected {} lines: not UTF-8", stats.invalid_utf8);
    }
    println!(
        "wrote {}: {} readings, {} candidates, {} bytes ({:.1?})",
        out.display(),
        stats.readings,
        stats.candidates,
        bytes.len(),
        started.elapsed()
    );
    Ok(())
}

fn open(path: &str) -> Result<FileDictionary, String> {
    FileDictionary::open(path).map_err(|e| format!("{path}: {e}"))
}

fn lookup(args: &[String]) -> Result<(), String> {
    let (file, readings) = args.split_first().ok_or("lookup needs FILE READING...")?;
    let dict = open(file)?;
    for reading in readings {
        println!("{reading}\t{}", dict.lookup(reading).join(","));
    }
    Ok(())
}

fn info(args: &[String]) -> Result<(), String> {
    let file = args.first().ok_or("info needs FILE")?;
    let dict = open(file)?;
    println!("readings: {}", dict.len());
    println!("bytes:    {}", dict.byte_len());
    println!("meta:     {}", dict.meta());
    Ok(())
}

/// Resident set size in KiB, where the OS makes it easy to read.
fn rss_kib() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
        line.split_whitespace().nth(1)?.parse().ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn show_rss(label: &str) {
    match rss_kib() {
        Some(kib) => println!("{label}: RSS {:.1} MiB", kib as f64 / 1024.0),
        None => println!("{label}: RSS not available on this OS"),
    }
}

fn bench(args: &[String]) -> Result<(), String> {
    let file = args.first().ok_or("bench needs FILE")?;
    let hashmap = args.iter().any(|a| a == "--hashmap");
    show_rss("start");

    let started = Instant::now();
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    println!("read file: {:.1?}", started.elapsed());
    let validating = Instant::now();
    let dict = FileDictionary::from_bytes(bytes).map_err(|e| format!("{file}: {e}"))?;
    println!(
        "validate: {:.1?}; load FileDictionary in total: {:.1?} ({} readings, {} bytes)",
        validating.elapsed(),
        started.elapsed(),
        dict.len(),
        dict.byte_len()
    );
    show_rss("after load");

    // Every 7th reading as hits, and the same readings with a suffix that
    // is never a key as misses.
    let readings: Vec<String> = (0..dict.len())
        .step_by(7)
        .filter_map(|i| dict.reading_at(i).map(str::to_string))
        .collect();
    let misses: Vec<String> = readings.iter().map(|r| format!("{r}\u{1}")).collect();
    for (label, keys) in [("hit", &readings), ("miss", &misses)] {
        let started = Instant::now();
        let mut found = 0usize;
        for key in keys {
            found += dict.lookup(key).len();
        }
        let elapsed = started.elapsed();
        println!(
            "{} {label} lookups: {:.1?} total, {:.0} ns each ({found} candidates)",
            keys.len(),
            elapsed,
            elapsed.as_nanos() as f64 / keys.len().max(1) as f64
        );
    }

    if hashmap {
        let started = Instant::now();
        let mut memory = MemoryDictionary::new();
        for i in 0..dict.len() {
            let Some(reading) = dict.reading_at(i) else {
                continue;
            };
            for candidate in dict.candidates(reading).unwrap_or_default() {
                memory.add(reading, candidate);
            }
        }
        println!(
            "load MemoryDictionary (from the same entries, no text parsing): {:.1?} ({} readings)",
            started.elapsed(),
            memory.len()
        );
        show_rss("after MemoryDictionary");
        let started = Instant::now();
        let mut found = 0usize;
        for key in &readings {
            found += memory.lookup(key).len();
        }
        let elapsed = started.elapsed();
        println!(
            "{} MemoryDictionary hit lookups: {:.1?} total, {:.0} ns each ({found} candidates)",
            readings.len(),
            elapsed,
            elapsed.as_nanos() as f64 / readings.len().max(1) as f64
        );
    }
    Ok(())
}
