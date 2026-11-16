//! picomatch constants (POSIX, `dot: true`), and the small utilities of its `utils.js`.

use super::units::{BACKSLASH, DOT, SLASH, cat, is, slice, w};
use super::{MAX_LENGTH, U};

pub(super) const DOT_LITERAL: &str = r"\.";
pub(super) const PLUS_LITERAL: &str = r"\+";
pub(super) const SLASH_LITERAL: &str = r"\/";
pub(super) const ONE_CHAR: &str = "(?=.)";
pub(super) const QMARK: &str = "[^/]";
pub(super) const NO_DOTS: &str = r"(?!(?:^|\/)\.{1,2}(?:\/|$))";
pub(super) const NO_DOT_SLASH: &str = r"(?!\.{0,1}(?:\/|$))";
pub(super) const NO_DOTS_SLASH: &str = r"(?!\.{1,2}(?:\/|$))";
pub(super) const STAR: &str = "[^/]*?";
/// `globstar({ dot: true })`.
pub(super) const GLOBSTAR: &str = r"(?:(?:(?!(?:^|\/)\.{1,2}(?:\/|$)).)*?)";
const REGEX_SPECIAL: &str = "-*+?.^${}(|)[]";

/// The POSIX classes picomatch knows, as the characters it puts in a class.
const POSIX_CLASSES: [(&str, &str); 14] = [
    ("alnum", "a-zA-Z0-9"),
    ("alpha", "a-zA-Z"),
    ("ascii", r"\x00-\x7F"),
    ("blank", r" \t"),
    ("cntrl", r"\x00-\x1F\x7F"),
    ("digit", "0-9"),
    ("graph", r"\x21-\x7E"),
    ("lower", "a-z"),
    ("print", r"\x20-\x7E "),
    ("punct", r##"\-!"#$%&'()\*+,./:;<=>?@[\]^_`{|}~"##),
    ("space", r" \t\r\n\v\f"),
    ("upper", "A-Z"),
    ("word", "A-Za-z0-9_"),
    ("xdigit", "A-Fa-f0-9"),
];

pub(super) fn posix_class(name: &[u16]) -> Option<&'static str> {
    POSIX_CLASSES.iter().find(|(n, _)| is(name, n)).map(|(_, s)| *s)
}

fn is_regex_special(c: u16) -> bool {
    REGEX_SPECIAL.encode_utf16().any(|s| s == c)
}

/// `utils.escapeRegex`.
pub(super) fn escape_regex(s: &[u16]) -> U {
    let mut out = U::with_capacity(s.len());
    for &c in s {
        if is_regex_special(c) {
            out.push(BACKSLASH);
        }
        out.push(c);
    }
    out
}

/// `utils.hasRegexChars`.
pub(super) fn has_regex_chars(s: &[u16]) -> bool {
    s.iter().any(|&c| is_regex_special(c))
}

/// `utils.escapeLast(input, char)`.
pub(super) fn escape_last(input: &[u16], c: u16) -> U {
    let mut from = input.len();
    loop {
        let found = slice(input, 0, from.saturating_add(1)).iter().rposition(|&x| x == c);
        let Some(idx) = found else {
            return input.to_vec();
        };
        if idx > 0 && input.get(idx - 1) == Some(&BACKSLASH) {
            from = idx - 1;
            continue;
        }
        return cat(&[slice(input, 0, idx), &[BACKSLASH], slice(input, idx, input.len())]);
    }
}

pub(super) fn length_error(len: usize) -> String {
    format!("Input length: {len}, exceeds maximum allowed length: {MAX_LENGTH}")
}

/// `REPLACEMENTS[input] || input`.
pub(super) fn apply_replacements(input: &[u16]) -> U {
    if is(input, "***") {
        w("*")
    } else if is(input, "**/**") || is(input, "**/**/**") {
        w("**")
    } else {
        input.to_vec()
    }
}

/// `utils.removePrefix`.
pub(super) fn remove_prefix(input: &[u16]) -> &[u16] {
    input.strip_prefix(&[DOT, SLASH][..]).unwrap_or(input)
}
