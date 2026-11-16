//! The repeated-extglob analysis (ReDoS guard) of parse.js.

use super::U;
use super::constants::escape_regex;
use super::units::{
    AT, BACKSLASH, LBRACKET, LPAREN, PIPE, PLUS_CH, QMARK_CH, QUOTE, RBRACKET, RPAREN, STAR_CH, ext,
    is_line_terminator, js_trim, slice, w,
};

/// Where a scan of an extglob's body stands as to escapes, quotes and classes.
#[derive(Default)]
struct Shelter {
    escaped: bool,
    quoted: bool,
    brackets: i64,
}

impl Shelter {
    /// Takes `ch` in; true when it is escaped, quoted, a quote, or part of a class, so has
    /// no meaning to the extglob's structure.
    fn shelters(&mut self, ch: u16) -> bool {
        if self.escaped {
            self.escaped = false;
            return true;
        }
        if ch == BACKSLASH {
            self.escaped = true;
            return true;
        }
        if ch == QUOTE {
            self.quoted = !self.quoted;
            return true;
        }
        if self.quoted {
            return true;
        }
        if ch == LBRACKET {
            self.brackets += 1;
            return true;
        }
        if ch == RBRACKET && self.brackets > 0 {
            self.brackets -= 1;
            return true;
        }
        self.brackets > 0
    }
}

fn split_top_level(input: &[u16]) -> Vec<U> {
    let mut parts = Vec::new();
    let mut shelter = Shelter::default();
    let mut paren = 0i64;
    let mut value = U::new();
    for &ch in input {
        if !shelter.shelters(ch) {
            if ch == LPAREN {
                paren += 1;
            } else if ch == RPAREN && paren > 0 {
                paren -= 1;
            } else if ch == PIPE && paren == 0 {
                parts.push(std::mem::take(&mut value));
                continue;
            }
        }
        value.push(ch);
    }
    parts.push(value);
    parts
}

fn is_plain_branch(branch: &[u16]) -> bool {
    let mut escaped = false;
    for &ch in branch {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == BACKSLASH {
            escaped = true;
            continue;
        }
        if "?*+@!()[]{}".encode_utf16().any(|s| s == ch) {
            return false;
        }
    }
    true
}

/// `normalizeSimpleBranch`; `None` is `undefined`.
fn normalize_simple_branch(branch: &[u16]) -> Option<U> {
    let mut value = js_trim(branch).to_vec();
    // `/^@\([^\\()[\]{}|]+\)$/`
    while value.len() >= 4
        && value.starts_with(&[AT, LPAREN])
        && value.last() == Some(&RPAREN)
        && slice(&value, 2, value.len() - 1).iter().all(|&c| !"\\()[]{}|".encode_utf16().any(|s| s == c))
    {
        value = slice(&value, 2, value.len() - 1).to_vec();
    }
    if !is_plain_branch(&value) {
        return None;
    }
    // `value.replace(/\\(.)/g, '$1')`
    let mut out = U::with_capacity(value.len());
    let mut i = 0;
    while let Some(&c) = value.get(i) {
        match value.get(i + 1) {
            Some(&n) if c == BACKSLASH && !is_line_terminator(n) => {
                out.push(n);
                i += 2;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Some(out)
}

fn has_repeated_char_prefix_overlap(branches: &[U]) -> bool {
    let values: Vec<U> =
        branches.iter().filter_map(|b| normalize_simple_branch(b)).filter(|v| !v.is_empty()).collect();
    for (i, a) in values.iter().enumerate() {
        for b in values.iter().skip(i + 1) {
            let Some(&first) = a.first() else { continue };
            if a.iter().all(|&c| c == first) && b.iter().all(|&c| c == first) {
                // Two runs of one character: one is always a prefix of the other.
                return true;
            }
        }
    }
    false
}

struct Repeated {
    kind: u16,
    body: U,
    end: usize,
}

fn parse_repeated_extglob(pattern: &[u16], require_end: bool) -> Option<Repeated> {
    let kind = *pattern.first()?;
    if (kind != PLUS_CH && kind != STAR_CH) || pattern.get(1) != Some(&LPAREN) {
        return None;
    }
    let end = closing_paren(pattern)?;
    if require_end && end != pattern.len() - 1 {
        return None;
    }
    Some(Repeated { kind, body: slice(pattern, 2, end).to_vec(), end })
}

/// Where the `)` that closes the `(` after an extglob's kind is.
fn closing_paren(pattern: &[u16]) -> Option<usize> {
    let mut shelter = Shelter::default();
    let mut paren = 0i64;
    for (i, &ch) in pattern.iter().enumerate().skip(1) {
        if shelter.shelters(ch) {
            continue;
        }
        if ch == LPAREN {
            paren += 1;
        } else if ch == RPAREN {
            paren -= 1;
            if paren == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn star_extglob_sequence_chars(pattern: &[u16]) -> Option<Vec<U>> {
    let mut index = 0;
    let mut chars = Vec::new();
    while index < pattern.len() {
        let m = parse_repeated_extglob(slice(pattern, index, pattern.len()), false)?;
        if m.kind != STAR_CH {
            return None;
        }
        let branches = split_top_level(&m.body);
        let [branch] = branches.as_slice() else {
            return None;
        };
        let branch = normalize_simple_branch(js_trim(branch))?;
        if branch.len() != 1 {
            return None;
        }
        chars.push(branch);
        index += m.end + 1;
    }
    if chars.is_empty() { None } else { Some(chars) }
}

fn repeated_extglob_recursion(pattern: &[u16]) -> usize {
    let mut depth = 0;
    let mut m = parse_repeated_extglob(js_trim(pattern), true);
    while let Some(found) = m {
        depth += 1;
        m = parse_repeated_extglob(js_trim(&found.body), true);
    }
    depth
}

pub(super) struct Analysis {
    pub(super) risky: bool,
    pub(super) safe_output: Option<U>,
}

/// `analyzeRepeatedExtglob(body, { dot: true })` (so `maxExtglobRecursion` is 0).
pub(super) fn analyze_repeated_extglob(body: &[u16]) -> Analysis {
    const RISKY: Analysis = Analysis { risky: true, safe_output: None };
    let branches: Vec<U> = split_top_level(body).iter().map(|b| js_trim(b).to_vec()).collect();
    if branches.len() > 1 && branches_overlap(&branches) {
        return RISKY;
    }
    let Some(safe) = safe_chars(&branches) else {
        return RISKY;
    };
    if !safe.saw_star_sequence {
        return Analysis { risky: false, safe_output: None };
    }
    if !safe.combinable {
        return RISKY;
    }
    Analysis { risky: true, safe_output: Some(repeated_class(safe.chars)) }
}

/// Alternatives that can match the same text in more than one way.
fn branches_overlap(branches: &[U]) -> bool {
    branches.iter().any(|b| b.is_empty())
        || branches.iter().any(|b| b.iter().all(|&c| c == STAR_CH || c == QMARK_CH))
        || has_repeated_char_prefix_overlap(branches)
}

struct SafeChars {
    chars: Vec<U>,
    saw_star_sequence: bool,
    combinable: bool,
}

/// The single characters the branches repeat or are; `None` when a branch that is
/// neither holds a repeated extglob.
fn safe_chars(branches: &[U]) -> Option<SafeChars> {
    let mut safe = SafeChars { chars: Vec::new(), saw_star_sequence: false, combinable: true };
    for branch in branches {
        if let Some(chars) = star_extglob_sequence_chars(branch) {
            safe.saw_star_sequence = true;
            safe.chars.extend(chars);
            continue;
        }
        if let Some(literal) = normalize_simple_branch(branch)
            && literal.len() == 1
        {
            safe.chars.push(literal);
            continue;
        }
        safe.combinable = false;
        if repeated_extglob_recursion(branch) > 0 {
            return None;
        }
    }
    Some(safe)
}

/// Any number of the characters, each written once: `c*` or `[abc]*`.
fn repeated_class(chars: Vec<U>) -> U {
    let mut unique: Vec<U> = Vec::new();
    for c in chars {
        if !unique.contains(&c) {
            unique.push(c);
        }
    }
    let mut source = match unique.as_slice() {
        [one] => escape_regex(one),
        many => {
            let mut s = w("[");
            for c in many {
                s.extend(escape_regex(c));
            }
            ext(&mut s, "]");
            s
        }
    };
    ext(&mut source, "*");
    source
}
