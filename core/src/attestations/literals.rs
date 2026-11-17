//! The bare literals in prose that a fact may bind: numbers and versions.

use crate::text::is_word;

/// The letter that may lead a number or version, as in `v1.2.3`; it is not part of the value.
const VERSION_MARK: char = 'v';

/// Every bare number (`3`, `-4`, `2.5`) and version (`0.1.0`, `1.2.3-rc.1`) in `text`, in
/// order, each without the `v` that may lead it (`v1.2.3` is `1.2.3`). A number is a match of
/// `/(?<![\w.$-])v?(-?\d+(?:\.\d+)?)(?![\w$]|\.\d)/g`; a version is three dot-separated
/// numbers with an optional pre-release (`-rc.1`) and build (`+abc`), under the same
/// boundaries.
pub fn literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        match literal_at(&chars, at) {
            Some((start, end)) => {
                out.push(chars[start..end].iter().collect());
                at = end;
            }
            None => at += 1,
        }
    }
    out
}

/// `text` as a number or version compares: without the `v` that leads `v1.2.3` or `v2`; any
/// other text as it is.
pub fn without_version_mark(text: &str) -> &str {
    match text.strip_prefix(VERSION_MARK) {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) && literals(text) == [rest] => rest,
        _ => text,
    }
}

/// Where the value of the literal at `at` starts, past a leading `v`, and where it ends.
fn literal_at(chars: &[char], at: usize) -> Option<(usize, usize)> {
    if !may_start(chars, at) {
        return None;
    }
    let start = if chars.get(at) == Some(&VERSION_MARK) && is_digit_at(chars, at + 1) { at + 1 } else { at };
    let end = version_at(chars, start).or_else(|| number_at(chars, start))?;
    Some((start, end))
}

fn number_at(chars: &[char], at: usize) -> Option<usize> {
    let start = if chars.get(at) == Some(&'-') { at + 1 } else { at };
    let int_end = digits_end(chars, start);
    // Greedy with backtracking: the longest integer part first, and for each, the longest
    // fraction, then none.
    for int in (start + 1..=int_end).rev() {
        if let Some(end) = fraction_end(chars, int) {
            return Some(end);
        }
        if ends_well(chars, int) {
            return Some(int);
        }
    }
    None
}

/// `\d+\.\d+\.\d+`, then `-` and dot-separated identifiers, then `+` and the same.
fn version_at(chars: &[char], at: usize) -> Option<usize> {
    let mut end = at;
    for part in 0..3 {
        if part > 0 {
            if chars.get(end) != Some(&'.') {
                return None;
            }
            end += 1;
        }
        let digits = digits_end(chars, end);
        if digits == end {
            return None;
        }
        end = digits;
    }
    for mark in ['-', '+'] {
        if chars.get(end) == Some(&mark) {
            end = identifiers_end(chars, end + 1).unwrap_or(end);
        }
    }
    ends_well(chars, end).then_some(end)
}

/// The end of `[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*` from `from`, if it matches at all.
fn identifiers_end(chars: &[char], from: usize) -> Option<usize> {
    let is_identifier = |k: usize| chars.get(k).is_some_and(|&c| c.is_ascii_alphanumeric() || c == '-');
    let mut end = from;
    while is_identifier(end) {
        end += 1;
        if chars.get(end) == Some(&'.') && is_identifier(end + 1) {
            end += 1;
        }
    }
    (end > from).then_some(end)
}

/// `(?<![\w.$-])`.
fn may_start(chars: &[char], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|k| chars.get(k))
        .is_none_or(|&before| !(is_word(before) || matches!(before, '.' | '$' | '-')))
}

fn is_digit_at(chars: &[char], k: usize) -> bool {
    chars.get(k).is_some_and(char::is_ascii_digit)
}

fn digits_end(chars: &[char], from: usize) -> usize {
    let mut end = from;
    while is_digit_at(chars, end) {
        end += 1;
    }
    end
}

/// `(?![\w$]|\.\d)` at `end`.
fn ends_well(chars: &[char], end: usize) -> bool {
    let next = chars.get(end).copied();
    !(next.is_some_and(|c| is_word(c) || c == '$') || (next == Some('.') && is_digit_at(chars, end + 1)))
}

/// After the integer part ending at `int`, the end of the longest fraction that ends well.
fn fraction_end(chars: &[char], int: usize) -> Option<usize> {
    if chars.get(int) != Some(&'.') || !is_digit_at(chars, int + 1) {
        return None;
    }
    let frac_end = digits_end(chars, int + 1);
    (int + 2..=frac_end).rev().find(|&end| ends_well(chars, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_found_as_the_recordings_hold() {
        assert_eq!(literals("At most 3 tries."), vec!["3"]);
        assert_eq!(literals("Version v2.1 or -4."), vec!["2.1", "-4"]);
        assert_eq!(literals("12a 7$ 8"), vec!["8"]);
        assert_eq!(literals("a-4 x"), Vec::<String>::new());
        assert_eq!(literals("3. next"), vec!["3"]);
        assert_eq!(literals("10.0"), vec!["10.0"]);
    }

    #[test]
    fn versions_are_found_whole() {
        assert_eq!(literals("1.5.3 and 2.5"), vec!["1.5.3", "2.5"]);
        assert_eq!(literals("Released as 0.1.0."), vec!["0.1.0"]);
        assert_eq!(literals("Try 1.2.3-rc.1, then 1.2.3+build.7."), vec!["1.2.3-rc.1", "1.2.3+build.7"]);
        assert_eq!(literals("1.2.3- next"), vec!["1.2.3"]);
        assert_eq!(literals("1.2.3.4 or 1.2.3x"), Vec::<String>::new());
    }

    #[test]
    fn a_leading_v_is_not_part_of_the_value() {
        assert_eq!(literals("Rust v1.89.0 and v2, not dev3, v-4 or vv1.0.0"), vec!["1.89.0", "2"]);
        assert_eq!(without_version_mark("v1.89.0"), "1.89.0");
        assert_eq!(without_version_mark("v2"), "2");
        assert_eq!(without_version_mark("1.89.0"), "1.89.0");
        assert_eq!(without_version_mark("v8 engine"), "v8 engine");
        assert_eq!(without_version_mark("version"), "version");
        assert_eq!(without_version_mark("v1.2.3.4"), "v1.2.3.4");
    }
}
