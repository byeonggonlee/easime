//! Try the Hangul engine from a terminal.
//!
//! Each input line is treated as a sequence of QWERTY key presses:
//!
//! ```text
//! $ echo "gksrmf" | cargo run -q -p easime-cli
//! 한글
//! ```
//!
//! Use `\b` (or the DEL character) for Backspace.

use engine_hangul::HangulEngine;
use ime_core::{InputEngine, Key, KeyEvent};
use std::io::{self, BufRead};

/// Feed `keys` through a fresh engine and return the resulting text.
fn convert(keys: &str) -> String {
    let mut engine = HangulEngine::new();
    let mut out = String::new();
    let mut chars = keys.chars().peekable();

    while let Some(c) = chars.next() {
        let key = match c {
            '\\' if chars.peek() == Some(&'b') => {
                chars.next();
                Key::Backspace
            }
            '\u{7f}' | '\u{8}' => Key::Backspace,
            ' ' => Key::Space,
            c => Key::Char(c),
        };
        let result = engine.process_key(KeyEvent::new(key));
        out.push_str(&result.commit);
        if !result.handled {
            match key {
                Key::Space => out.push(' '),
                Key::Char(c) => out.push(c),
                // Backspace with nothing composed deletes already committed text.
                Key::Backspace => {
                    out.pop();
                }
                _ => {}
            }
        }
    }
    out.push_str(&engine.flush());
    out
}

fn main() {
    for line in io::stdin().lock().lines() {
        match line {
            Ok(line) => println!("{}", convert(&line)),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::convert;

    #[test]
    fn converts_words() {
        assert_eq!(convert("gksrmf"), "한글");
        assert_eq!(convert("dkssudgktpdy"), "안녕하세요");
    }

    #[test]
    fn keeps_spaces_digits_and_punctuation() {
        assert_eq!(convert("gksrmf rhd 123."), "한글 공 123.");
    }

    #[test]
    fn backspace_works_inside_and_after_composition() {
        assert_eq!(convert("gks\\b"), "하");
        assert_eq!(convert("gk gk\\b\\b\\b\\b"), "");
        assert_eq!(convert("gk 1\\bgk"), "하 하");
    }
}
