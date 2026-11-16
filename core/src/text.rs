//! String operations with JavaScript's semantics, kept on purpose: they decide how text is
//! split, collapsed and sorted in docattest's output, which must be the same whichever
//! adapter serves a project (PROTOCOL.md §2 states the order).
//!
//! JavaScript's `\s` and `String.prototype.trim` use one whitespace set (WhiteSpace and
//! LineTerminator): it contains U+FEFF and not U+0085, which is not Rust's `char::is_whitespace`.
//! JavaScript compares strings by UTF-16 code units, which orders astral characters before
//! U+E000..U+FFFF, unlike Rust's byte (code point) order. `\w` and `\b` without the `u` flag
//! are ASCII-only.

use std::cmp::Ordering;

/// Whether `c` matches JavaScript's `\s`.
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// Whether `c` matches JavaScript's `\w` (no `u` flag): `[A-Za-z0-9_]`.
pub fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `s.trim()`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `s.replace(/\s+/g, ' ')`.
pub fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if is_js_space(c) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// `s.split(/\s+/).filter(Boolean)`.
pub fn words(s: &str) -> Vec<&str> {
    s.split(is_js_space).filter(|w| !w.is_empty()).collect()
}

/// `text.split(/\r?\n/)`.
pub fn lines(text: &str) -> Vec<&str> {
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect()
}

/// The first line of `text.split(/\r?\n/)`.
pub fn first_line(text: &str) -> &str {
    let line = text.split('\n').next().unwrap_or("");
    line.strip_suffix('\r').unwrap_or(line)
}

/// `a < b ? -1 : a > b ? 1 : 0` on JavaScript strings: UTF-16 code unit order.
///
/// ```
/// use docattest::text::compare_utf16;
/// use std::cmp::Ordering;
///
/// assert_eq!(compare_utf16("a", "b"), Ordering::Less);
/// assert_eq!(compare_utf16("b", "a\u{1f600}"), Ordering::Greater);
/// assert_eq!(compare_utf16("a", "a"), Ordering::Equal);
/// ```
pub fn compare_utf16(a: &str, b: &str) -> Ordering {
    // Equal prefixes compare equal in both encodings; only the first difference matters.
    let mut x = a.chars();
    let mut y = b.chars();
    loop {
        match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) if p == q => continue,
            (Some(p), Some(q)) => return utf16_key(p).cmp(&utf16_key(q)),
        }
    }
}

// The first UTF-16 unit decides between two different characters, and only differs from
// code point order when one of them is astral (a surrogate, 0xD800..0xDBFF, comes first).
fn utf16_key(c: char) -> (u16, u16) {
    let mut buf = [0u16; 2];
    let units = c.encode_utf16(&mut buf);
    (units[0], units.get(1).copied().unwrap_or(0))
}

/// The first `n` characters of `s`, all of it when it is shorter.
pub fn prefix(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map_or(s, |(at, _)| &s[..at])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_javascripts_set() {
        assert_eq!(trim("\u{FEFF} a \u{A0}"), "a");
        assert_eq!(trim("\u{85}a"), "\u{85}a");
        assert_eq!(collapse_spaces(" a \n\t b "), " a b ");
        assert_eq!(words("  a  b\u{3000}c "), vec!["a", "b", "c"]);
        assert_eq!(lines("a\r\nb\nc\r"), vec!["a", "b", "c"]);
    }

    #[test]
    fn strings_compare_by_utf16_units() {
        assert_eq!(compare_utf16("a", "b"), Ordering::Less);
        assert_eq!(compare_utf16("ab", "a"), Ordering::Greater);
        // U+1F600 is D83D DE00 in UTF-16, which sorts before U+FF01.
        assert_eq!(compare_utf16("\u{1F600}", "\u{FF01}"), Ordering::Less);
        assert_eq!("\u{1F600}".cmp("\u{FF01}"), Ordering::Greater);
    }

    #[test]
    fn prefixes_keep_whole_characters() {
        assert_eq!(prefix("ab\u{1F600}c", 3), "ab\u{1F600}");
        assert_eq!(prefix("abc", 2), "ab");
        assert_eq!(prefix("abc", 5), "abc");
    }
}
