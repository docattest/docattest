//! The bare literals in prose that a fact may bind: numbers and versions.

use crate::text::is_word;

/// Every bare number (`3`, `-4`, `2.5`) and version (`0.1.0`, `1.2.3-rc.1`) in `text`, in
/// order. A number is a match of `/(?<![\w.$-])(-?\d+(?:\.\d+)?)(?![\w$]|\.\d)/g`, as the
/// recordings hold; a version is three dot-separated numbers with an optional pre-release
/// (`-rc.1`) and build (`+abc`), under the same boundaries.
pub fn literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        match version_at(&chars, at).or_else(|| number_at(&chars, at)) {
            Some(end) => {
                out.push(chars[at..end].iter().collect());
                at = end;
            }
            None => at += 1,
        }
    }
    out
}

fn number_at(chars: &[char], at: usize) -> Option<usize> {
    if !may_start(chars, at) {
        return None;
    }
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
    if !may_start(chars, at) {
        return None;
    }
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
        assert_eq!(literals("Version v2.1 or -4."), vec!["-4"]);
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
        assert_eq!(literals("v1.2.3 or 1.2.3.4 or 1.2.3x"), Vec::<String>::new());
    }
}
