//! How two pieces of prose are compared. Deliberately conservative: a guard that fires on an honest
//! edit gets switched off, so "the same sentence" has to mean nearly that.
//!
//! The rules, which the recordings hold: words are runs of letters, digits and `_` after
//! lower-casing (general categories L and N, not Rust's `is_alphabetic`), `\d` and `\w` are
//! ASCII, whitespace is the set of [`crate::text`], sorting is by UTF-16 code units, and a
//! quotation is cut at 59 UTF-16 units, which can split an astral character in two.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::text::{collapse_spaces, compare_utf16, is_js_space, prefix, trim};

/// Word-level similarity at or above which a reworded sentence counts as the same one.
pub const SIMILAR: f64 = 0.8;

fn word_regex() -> &'static fancy_regex::Regex {
    static WORD: OnceLock<fancy_regex::Regex> = OnceLock::new();
    WORD.get_or_init(|| fancy_regex::Regex::new(r"[\p{L}\p{N}_]+").expect("a constant pattern compiles"))
}

/// Lower-case words, code spans included.
pub fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    word_regex().find_iter(&lower).flatten().map(|m| m.as_str().to_string()).collect()
}

/// The matches of ``/`+([^`]+?)`+/g``: each span's content, and the text with every match
/// replaced by a space.
fn code_spans(text: &str) -> (Vec<&str>, String) {
    let mut spans = Vec::new();
    let mut rest = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(open) = text[at..].find('`').map(|k| at + k) {
        let content = open + text[open..].bytes().take_while(|b| *b == b'`').count();
        // Without a closing run no later start can match either.
        let Some(close) = text[content..].find('`').map(|k| content + k) else { break };
        let end = close + text[close..].bytes().take_while(|b| *b == b'`').count();
        spans.push(&text[content..close]);
        rest.push_str(&text[at..open]);
        rest.push(' ');
        at = end;
    }
    rest.push_str(&text[at..]);
    (spans, rest)
}

/// The matches of `/\d+(?:\.\d+)?/g`.
fn numbers(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let digits = |from: usize| bytes[from..].iter().take_while(|b| b.is_ascii_digit()).count();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let whole = digits(at);
        if whole == 0 {
            at += 1;
            continue;
        }
        let mut end = at + whole;
        if bytes.get(end) == Some(&b'.') {
            let fraction = digits(end + 1);
            if fraction > 0 {
                end += 1 + fraction;
            }
        }
        out.push(&text[at..end]);
        at = end;
    }
    out
}

/// Code spans and bare numbers, sorted: the parts of a sentence a fact pins.
pub fn literals(text: &str) -> Vec<String> {
    let (spans, rest) = code_spans(text);
    let mut all: Vec<String> = spans.into_iter().map(|s| trim(s).to_string()).collect();
    all.extend(numbers(&rest).into_iter().map(str::to_string));
    all.sort_by(|a, b| compare_utf16(a, b));
    all
}

/// Enough of a sentence to be a claim: three words, or two around a literal.
pub fn substantive(text: &str) -> bool {
    let n = words(text).len();
    n >= 3 || (n >= 2 && !literals(text).is_empty())
}

/// Dice coefficient over word multisets: 1 for the same words, 0 for none in common.
pub fn dice(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for w in a {
        *counts.entry(w.as_str()).or_default() += 1;
    }
    let mut shared = 0usize;
    for w in b {
        if let Some(left) = counts.get_mut(w.as_str())
            && *left > 0
        {
            shared += 1;
            *left -= 1;
        }
    }
    (2 * shared) as f64 / (a.len() + b.len()) as f64
}

/// A light rewording: the same literals and nearly the same words.
pub fn similar(a: &str, b: &str) -> bool {
    if literals(a).join("\0") != literals(b).join("\0") {
        return false;
    }
    dice(&words(a), &words(b)) >= SIMILAR
}

/// Whitespace collapsed, for comparing code and expressions.
pub fn squash(text: &str) -> String {
    trim(&collapse_spaces(text)).to_string()
}

/// A claim quoted for a message: whole when it is at most 60 characters, else cut to fit
/// with `…`.
pub fn quote(text: &str) -> String {
    const MAX: usize = 60;
    if text.chars().count() <= MAX {
        return format!("\"{text}\"");
    }
    format!("\"{}…\"", prefix(text, MAX - 1).trim_end_matches(is_js_space))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn words_are_letters_numbers_and_underscores_lower_cased() {
        assert_eq!(
            words("A call is tried at most `3` times."),
            strings(&["a", "call", "is", "tried", "at", "most", "3", "times"])
        );
        assert_eq!(words("MAX_TRIES, Größe 日本 x²"), strings(&["max_tries", "größe", "日本", "x²"]));
        // A combining mark is not a letter (it is alphabetic, which is why `\p{L}` is used).
        assert_eq!(words("e\u{301}t"), strings(&["e", "t"]));
        assert_eq!(words("İstanbul"), strings(&["i", "stanbul"]));
        assert!(words("… — !").is_empty());
    }

    #[test]
    fn literals_are_code_spans_and_bare_numbers_sorted() {
        assert_eq!(literals("At most `3` tries, 2.5 s, `` a`b ``"), strings(&["2.5", "3", "a"]));
        assert_eq!(literals("`b` then `a` then 10 and 9."), strings(&["10", "9", "a", "b"]));
        assert_eq!(literals("an `unclosed span and 7"), strings(&["7"]));
        assert_eq!(literals("`1` 1. 1.x .5"), strings(&["1", "1", "1", "5"]));
        assert!(literals("no literal").is_empty());
    }

    #[test]
    fn substantive_needs_three_words_or_two_around_a_literal() {
        assert!(substantive("one two three"));
        assert!(substantive("limit `3`"));
        assert!(!substantive("two words"));
        assert!(!substantive("`3`"));
    }

    #[test]
    fn dice_counts_shared_words_as_a_multiset() {
        let w = |s: &str| words(s);
        assert_eq!(dice(&[], &[]), 1.0);
        assert_eq!(dice(&w("a b"), &[]), 0.0);
        assert_eq!(dice(&w("a a b"), &w("a b b")), 4.0 / 6.0);
        assert_eq!(dice(&w("a b c d"), &w("a b c e")), 0.75);
    }

    #[test]
    fn similar_is_the_same_literals_and_nearly_the_same_words() {
        assert!(similar("A call is tried at most `3` times.", "A call is attempted at most `3` times."));
        assert!(!similar("A call is tried at most `3` times.", "A call is tried at most `4` times."));
        assert!(!similar(
            "A call is tried at most `3` times.",
            "Calls are retried up to `3` times in total."
        ));
    }

    #[test]
    fn squash_collapses_javascript_whitespace() {
        assert_eq!(squash("  a \n\t b\u{A0}c \u{FEFF}"), "a b c");
        assert_eq!(squash("a\u{85}b"), "a\u{85}b");
    }

    #[test]
    fn quotations_are_cut_at_sixty_characters() {
        assert_eq!(quote("short"), "\"short\"");
        let sixty = "x".repeat(60);
        assert_eq!(quote(&sixty), format!("\"{sixty}\""));
        let long = format!("{} tail", "x".repeat(60));
        assert_eq!(quote(&long), format!("\"{}…\"", "x".repeat(59)));
        let astral = format!("{}\u{1F600}\u{1F600}\u{1F600}", "x".repeat(58));
        assert_eq!(quote(&astral), format!("\"{}\u{1F600}…\"", "x".repeat(58)));
    }
}
