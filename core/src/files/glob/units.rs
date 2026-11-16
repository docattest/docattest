//! UTF-16 helpers: picomatch works on JavaScript strings, so the port works on code units.

use std::borrow::Cow;

use super::{SURROGATE_BASE, U};

pub(super) const BACKSLASH: u16 = b'\\' as u16;
pub(super) const SLASH: u16 = b'/' as u16;
pub(super) const DOT: u16 = b'.' as u16;
pub(super) const STAR_CH: u16 = b'*' as u16;
pub(super) const QMARK_CH: u16 = b'?' as u16;
pub(super) const BANG: u16 = b'!' as u16;
pub(super) const PLUS_CH: u16 = b'+' as u16;
pub(super) const AT: u16 = b'@' as u16;
pub(super) const LPAREN: u16 = b'(' as u16;
pub(super) const RPAREN: u16 = b')' as u16;
pub(super) const LBRACKET: u16 = b'[' as u16;
pub(super) const RBRACKET: u16 = b']' as u16;
pub(super) const LBRACE: u16 = b'{' as u16;
pub(super) const RBRACE: u16 = b'}' as u16;
pub(super) const COMMA: u16 = b',' as u16;
pub(super) const PIPE: u16 = b'|' as u16;
pub(super) const COLON: u16 = b':' as u16;
pub(super) const QUOTE: u16 = b'"' as u16;
pub(super) const DASH: u16 = b'-' as u16;
pub(super) const CARET: u16 = b'^' as u16;
pub(super) const DOLLAR: u16 = b'$' as u16;
pub(super) const LT: u16 = b'<' as u16;
pub(super) const GT: u16 = b'>' as u16;
pub(super) const EQUALS: u16 = b'=' as u16;
pub(super) const SEMI: u16 = b';' as u16;

pub(super) fn w(s: &str) -> U {
    s.encode_utf16().collect()
}

pub(super) fn is(value: &[u16], s: &str) -> bool {
    value.iter().copied().eq(s.encode_utf16())
}

pub(super) fn cat(parts: &[&[u16]]) -> U {
    parts.concat()
}

pub(super) fn ext(v: &mut U, s: &str) {
    v.extend(s.encode_utf16());
}

/// `s.slice(start, end)` with JavaScript's clamping of non-negative indices.
pub(super) fn slice(s: &[u16], start: usize, end: usize) -> &[u16] {
    let end = end.min(s.len());
    s.get(start.min(end)..end).unwrap_or_default()
}

/// `s = s.slice(0, -n)`; note that `-0` slices to the empty string.
pub(super) fn drop_last(s: &mut U, n: usize) {
    let keep = if n == 0 { 0 } else { s.len().saturating_sub(n) };
    s.truncate(keep);
}

pub(super) fn is_word_unit(c: u16) -> bool {
    u8::try_from(c).is_ok_and(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(super) fn is_line_terminator(c: u16) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

/// JavaScript `WhiteSpace` or `LineTerminator`, as `String.prototype.trim` and `\s` see them.
fn is_js_space(c: u16) -> bool {
    matches!(
        c,
        0x09..=0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F
            | 0x3000 | 0xFEFF
    )
}

pub(super) fn js_trim(s: &[u16]) -> &[u16] {
    let start = s.iter().position(|&c| !is_js_space(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|&c| !is_js_space(c)).map_or(start, |i| i + 1);
    slice(s, start, end)
}

/// The character standing for UTF-16 code unit `u` in transcoded text.
pub(super) fn unit_code(u: u16) -> u32 {
    if (0xD800..=0xDFFF).contains(&u) { SURROGATE_BASE + u32::from(u - 0xD800) } else { u32::from(u) }
}

/// The path with each UTF-16 code unit as one `char` (borrowed when it is all BMP).
pub(super) fn transcode(path: &str) -> Cow<'_, str> {
    if path.chars().all(|c| u32::from(c) < 0x1_0000) {
        return Cow::Borrowed(path);
    }
    Cow::Owned(path.encode_utf16().filter_map(|u| char::from_u32(unit_code(u))).collect())
}
