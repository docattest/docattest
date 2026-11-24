//! When a page literal is close enough to a surface value to say so. Precision over recall: every
//! rule here would rather stay silent than make a suggestion that is wrong.

use std::collections::HashSet;

use super::{Observation, Status, distinct};
use crate::attestations::literals::literals;
use crate::text::{collapse_spaces, compare_utf16, trim};

/// How a candidate compares with one surface value. Higher `score`, then `weight`, wins.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub status: Status,
    pub missing: Vec<String>,
    pub unexpected: Vec<String>,
    pub score: f64,
    pub weight: usize,
}

/// Shared items over all distinct items, page and code together (Jaccard).
pub const MIN_OVERLAP: f64 = 0.5;
/// Shared items that are not trivial (one character, a digit, a boolean-like word). With
/// exactly this many shared, at most one item may differ.
pub const MIN_SHARED: usize = 2;
/// Collapsed length a block must reach before it can match a string (UTF-16 units).
pub const MIN_BLOCK_LENGTH: usize = 16;
/// Non-empty lines each side needs before a block can nearly match a string.
pub const MIN_BLOCK_LINES: usize = 3;

/// Digits a number needs before it is evidence on its own.
pub const MIN_VALUE_DIGITS: usize = 3;
/// Characters text needs before it is evidence on its own.
pub const MIN_VALUE_LENGTH: usize = 4;

const TRIVIAL: [&str; 9] = ["true", "false", "null", "undefined", "yes", "no", "on", "off", "none"];

/// Whether one value on its own could hardly equal a surface value by chance: a number of
/// at least three digits, or text of at least four characters whose core (after trimming
/// leading/trailing punctuation) is not a plain word or short number — because it holds
/// internal punctuation or digits (`DOCATTEST_CORE`, `batch-large`, `/etc/ld.so.preload`,
/// `:8080`, `v1.2`) or camelCase (`maxRetries`). Single-word sigils (`/proc`, `.git`,
/// `--help`) are still plain words and match unrelated prose too often.
pub fn is_distinctive(value: &str) -> bool {
    if literals(value).first().is_some_and(|number| number == value) {
        return value.chars().filter(char::is_ascii_digit).count() >= MIN_VALUE_DIGITS;
    }
    if value.chars().count() < MIN_VALUE_LENGTH || is_trivial(value) {
        return false;
    }
    let core = value.trim_matches(|c: char| !c.is_alphanumeric());
    if core.is_empty() {
        return false;
    }
    if core.chars().all(|c| c.is_ascii_digit()) {
        return core.len() >= MIN_VALUE_DIGITS;
    }
    let chars: Vec<char> = core.chars().collect();
    let camel = chars.windows(2).any(|pair| pair[0].is_lowercase() && pair[1].is_uppercase());
    camel || !chars.iter().all(|c| c.is_alphabetic())
}

/// Too trivial to be evidence: shorter than two characters, one digit (negative or not), or
/// one of [`TRIVIAL`] in any ASCII case. No non-ASCII character is folded onto an ASCII one.
fn is_trivial(item: &str) -> bool {
    if item.chars().count() < 2 || TRIVIAL.iter().any(|word| item.eq_ignore_ascii_case(word)) {
        return true;
    }
    let digits = item.strip_prefix('-').unwrap_or(item);
    digits.len() == 1 && digits.bytes().all(|b| b.is_ascii_digit())
}

fn sorted(mut items: Vec<String>) -> Vec<String> {
    items.sort_by(|a, b| compare_utf16(a, b));
    items
}

/// `text.replace(/\s+/g, ' ').trim()`.
pub fn collapse(text: &str) -> String {
    trim(&collapse_spaces(text)).to_string()
}

/// Compare a page's items with a surface list as sets, as a fact compares them.
pub fn compare_sets(page_items: &[String], code_items: &[String]) -> Option<Verdict> {
    let page = distinct(page_items.iter().map(String::as_str).filter(|item| !item.is_empty()));
    let code = distinct(code_items.iter().map(String::as_str));
    if page.len() < 2 || code.len() < 2 {
        return None;
    }
    let in_page: HashSet<&str> = page.iter().copied().collect();
    let in_code: HashSet<&str> = code.iter().copied().collect();
    let shared: Vec<&str> = page.iter().copied().filter(|item| in_code.contains(item)).collect();
    if shared.iter().filter(|item| !is_trivial(item)).count() < MIN_SHARED {
        return None;
    }
    let union = page.len() + code.len() - shared.len();
    let score = shared.len() as f64 / union as f64;
    if score < MIN_OVERLAP {
        return None;
    }
    let pick = |from: &[&str], other: &HashSet<&str>| -> Vec<String> {
        sorted(from.iter().filter(|item| !other.contains(*item)).map(|item| (*item).to_string()).collect())
    };
    let missing = pick(&code, &in_page);
    let unexpected = pick(&page, &in_code);
    // Two shared items are thin evidence: with only two, allow a single difference.
    if shared.len() <= MIN_SHARED && missing.len() + unexpected.len() > 1 {
        return None;
    }
    let status = if missing.is_empty() && unexpected.is_empty() { Status::Match } else { Status::Drift };
    let weight = shared.iter().map(|item| item.chars().count()).sum();
    Some(Verdict { status, missing, unexpected, score, weight })
}

/// Compare a fenced block with a string: equal after collapsing whitespace is a match; sharing
/// enough lines, compared like list items, is drift.
pub fn compare_block(block: &str, value: &Observation) -> Option<Verdict> {
    if !value.is_text {
        return None;
    }
    let page = collapse(block);
    let code = collapse(&value.text);
    if code.chars().count() < MIN_BLOCK_LENGTH {
        return None;
    }
    if page == code {
        return Some(Verdict {
            status: Status::Match,
            missing: Vec::new(),
            unexpected: Vec::new(),
            score: 1.0,
            weight: 0,
        });
    }
    let page_lines = block_lines(block);
    let code_lines = block_lines(&value.text);
    if page_lines.len() < MIN_BLOCK_LINES || code_lines.len() < MIN_BLOCK_LINES {
        return None;
    }
    compare_sets(&page_lines, &code_lines)
}

fn block_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .map(collapse)
        .filter(|line| line.chars().count() >= 3 && line.bytes().any(|b| b.is_ascii_alphanumeric()))
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    /// An observation of a string, as an adapter reports one.
    pub(crate) fn observe_text(text: &str) -> Observation {
        Observation { text: text.into(), items: None, description: format!("text {text:?}"), is_text: true }
    }

    /// An observation of a list of strings, as an adapter reports one.
    pub(crate) fn observe_list(items: &[&str]) -> Observation {
        Observation {
            text: items.join(","),
            items: Some(strings(items)),
            description: "a list".into(),
            is_text: false,
        }
    }

    fn sets(page: &[&str], code: &[&str]) -> Option<Verdict> {
        compare_sets(&strings(page), &strings(code))
    }

    #[test]
    fn matches_equal_sets_whatever_the_order_or_duplicates() {
        let v = sets(&["b1", "a1", "a1"], &["a1", "b1"]).unwrap();
        assert_eq!(v.status, Status::Match);
        assert!(v.missing.is_empty() && v.unexpected.is_empty());
        assert_eq!(v.score, 1.0);
    }

    #[test]
    fn reports_drift_in_the_fact_vocabulary_sorted() {
        let v = sets(
            &["API_TOKEN", "REGION", "PORT"],
            &["API_TOKEN", "REGION", "PORT", "RATE", "B_KEY", "A_KEY"],
        )
        .unwrap();
        assert_eq!(v.status, Status::Drift);
        assert_eq!(v.missing, strings(&["A_KEY", "B_KEY", "RATE"]));
        assert!(v.unexpected.is_empty());
        assert_eq!(v.score, 0.5);
    }

    #[test]
    fn stays_silent_below_half_overlap() {
        assert_eq!(sets(&["a1", "a2", "a3"], &["a1", "a2", "a3", "a4", "a5", "a6", "a7"]), None);
        assert_eq!(sets(&["id", "name", "email", "age"], &["id", "createdAt", "updatedAt", "owner"]), None);
    }

    #[test]
    fn allows_one_difference_only_when_two_items_are_shared() {
        assert_eq!(
            sets(&["read", "write"], &["read", "write", "admin"]).unwrap().missing,
            strings(&["admin"])
        );
        assert_eq!(sets(&["read", "write", "billing"], &["read", "write", "admin"]), None);
    }

    #[test]
    fn needs_two_shared_items_that_are_not_trivial_and_two_items_each_side() {
        assert_eq!(sets(&["1", "2", "x"], &["1", "2", "x"]), None);
        assert_eq!(sets(&["yes", "no", "on"], &["yes", "no", "on"]), None);
        assert_eq!(sets(&["alpha"], &["alpha"]), None);
        assert_eq!(sets(&["80", "443"], &["80", "443"]).unwrap().status, Status::Match);
    }

    #[test]
    fn ignores_empty_page_items() {
        assert_eq!(sets(&["eu", "", "us"], &["eu", "us"]).unwrap().status, Status::Match);
    }

    #[test]
    fn trivial_items_are_one_character_or_a_common_word() {
        for item in ["", "x", "7", "-7", "TRUE", "None", "é", "Off", "\u{1F600}"] {
            assert!(is_trivial(item), "{item}");
        }
        for item in ["42", "--1", "-12", "+1", "nope", "７７"] {
            assert!(!is_trivial(item), "{item}");
        }
    }

    #[test]
    fn weight_counts_characters_and_order_uses_utf16() {
        let v = sets(&["a\u{1F600}", "bb", "cc", "\u{FF01}x"], &["a\u{1F600}", "bb", "cc"]).unwrap();
        assert_eq!(v.weight, 6);
        let v = sets(&["aa", "bb", "cc"], &["aa", "bb", "cc", "\u{FF01}", "\u{1F600}"]).unwrap();
        assert_eq!(v.missing, strings(&["\u{1F600}", "\u{FF01}"]));
    }

    #[test]
    fn distinctive_values_are_long_numbers_and_text_that_is_not_a_plain_word() {
        for value in [
            "8080",
            "1.23",
            "-100",
            ":8080",
            "DOCATTEST_CORE",
            "batch-large",
            "maxRetries",
            "a b c",
            "v1.2",
            "/etc/ld.so.preload",
            "/api/v1",
            "--dry-run",
        ] {
            assert!(is_distinctive(value), "{value}");
        }
        for value in [
            "1",
            "90",
            "1.5",
            "queue",
            "Production",
            "JSON",
            "Größe",
            "a-b",
            "none",
            "True",
            "/proc",
            "/tmp",
            ".git",
            "--help",
            "----",
            "--12",
        ] {
            assert!(!is_distinctive(value), "{value}");
        }
    }

    fn template() -> Observation {
        observe_text("name: demo\nregion: eu\nport: 8080\nlog: info\n")
    }

    #[test]
    fn matches_after_collapsing_whitespace() {
        let v = compare_block("name:   demo\n  region: eu\nport: 8080\nlog: info", &template()).unwrap();
        assert_eq!(v.status, Status::Match);
    }

    #[test]
    fn is_drift_when_enough_lines_are_shared() {
        let v = compare_block("name: demo\nregion: eu\nport: 8080\n", &template()).unwrap();
        assert_eq!(v.status, Status::Drift);
        assert_eq!(v.missing, strings(&["log: info"]));
        assert!(v.unexpected.is_empty());
    }

    #[test]
    fn never_compares_with_short_strings_numbers_or_lists() {
        assert_eq!(compare_block("hello world", &observe_text("hello world")), None);
        let number = Observation {
            text: "1234567890123456".into(),
            items: None,
            description: "string that is a number, says its description".into(),
            is_text: false,
        };
        assert_eq!(compare_block("1234567890123456", &number), None);
        assert_eq!(compare_block("a\nb\nc", &observe_list(&["a", "b", "c"])), None);
    }

    #[test]
    fn needs_three_lines_each_side_for_drift() {
        assert_eq!(compare_block("name: demo\nregion: eu", &template()), None);
    }

    #[test]
    fn block_length_is_counted_in_characters() {
        // Sixteen astral characters: 16 characters, 32 UTF-16 units, 64 bytes.
        let text = "\u{1F600}".repeat(MIN_BLOCK_LENGTH);
        assert!(compare_block(&text, &observe_text(&text)).is_some());
        let short = "\u{1F600}".repeat(MIN_BLOCK_LENGTH - 1);
        assert_eq!(compare_block(&short, &observe_text(&short)), None);
    }
}
