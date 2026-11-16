//! Character classes: code unit ranges, how a class is written for fancy_regex, and the
//! translator's grammar for `[…]`.

use super::{ClassAtom, Frag, RegexError, Rx, Translator};
use crate::files::glob::units::{BACKSLASH, CARET, DASH, RBRACKET, unit_code};

pub(super) const WORD_CLASS: &str = "[0-9A-Za-z_]";

fn word_ranges() -> Vec<(u16, u16)> {
    vec![(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)]
}

fn space_ranges() -> Vec<(u16, u16)> {
    vec![
        (0x09, 0x0D),
        (0x20, 0x20),
        (0xA0, 0xA0),
        (0x1680, 0x1680),
        (0x2000, 0x200A),
        (0x2028, 0x2029),
        (0x202F, 0x202F),
        (0x205F, 0x205F),
        (0x3000, 0x3000),
        (0xFEFF, 0xFEFF),
    ]
}

fn normalize(mut ranges: Vec<(u16, u16)>) -> Vec<(u16, u16)> {
    ranges.sort_unstable();
    let mut out: Vec<(u16, u16)> = Vec::with_capacity(ranges.len());
    for (a, b) in ranges {
        match out.last_mut() {
            Some(last) if u32::from(a) <= u32::from(last.1) + 1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

pub(super) fn complement(ranges: Vec<(u16, u16)>) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    let mut next: u32 = 0;
    for (a, b) in normalize(ranges) {
        if u32::from(a) > next
            && let Ok(lo) = u16::try_from(next)
        {
            out.push((lo, a - 1));
        }
        next = u32::from(b) + 1;
    }
    if let Ok(lo) = u16::try_from(next) {
        out.push((lo, 0xFFFF));
    }
    out
}

fn hex(code: u32) -> String {
    format!("\\x{{{code:X}}}")
}

pub(super) fn literal(u: u16) -> String {
    match u8::try_from(u) {
        Ok(b) if b.is_ascii_alphanumeric() => char::from(b).to_string(),
        _ => hex(unit_code(u)),
    }
}

/// A class over code units, spelled over transcoded characters.
pub(super) fn class_source(ranges: Vec<(u16, u16)>) -> String {
    let mut s = String::from("[");
    let mut any = false;
    for (a, b) in normalize(ranges) {
        for (lo, hi) in [(0x0000, 0xD7FF), (0xD800, 0xDFFF), (0xE000, 0xFFFF)] {
            let (x, y) = (a.max(lo), b.min(hi));
            if x > y {
                continue;
            }
            any = true;
            s.push_str(&hex(unit_code(x)));
            if y > x {
                s.push('-');
                s.push_str(&hex(unit_code(y)));
            }
        }
    }
    if !any {
        // Nothing transcoded text can contain: an empty class never matches.
        s.push_str(&hex(0x10_FFFF));
    }
    s.push(']');
    s
}

impl Translator<'_> {
    pub(super) fn class(&mut self) -> Rx<Frag> {
        self.i += 1;
        let negated = self.cur() == Some(CARET);
        if negated {
            self.i += 1;
        }
        let mut ranges: Vec<(u16, u16)> = Vec::new();
        loop {
            if self.class_end()? {
                break;
            }
            if self.class_member(&mut ranges)? {
                break;
            }
        }
        let ranges = if negated { complement(ranges) } else { ranges };
        Ok(Frag::atom(class_source(ranges)))
    }

    /// Reads the `]` that ends the class (true); a class must end.
    fn class_end(&mut self) -> Rx<bool> {
        match self.cur() {
            None => Err(RegexError::Syntax),
            Some(RBRACKET) => {
                self.i += 1;
                Ok(true)
            }
            Some(_) => Ok(false),
        }
    }

    /// Reads one atom or range into `ranges`; true when a `-` before `]` ended the class.
    fn class_member(&mut self, ranges: &mut Vec<(u16, u16)>) -> Rx<bool> {
        let first = self.class_atom()?;
        if self.cur() != Some(DASH) {
            add_atom(ranges, &first);
            return Ok(false);
        }
        self.i += 1;
        if self.class_end()? {
            add_atom(ranges, &first);
            ranges.push((DASH, DASH));
            return Ok(true);
        }
        let second = self.class_atom()?;
        match (&first, &second) {
            (ClassAtom::Unit(a), ClassAtom::Unit(b)) => {
                if a > b {
                    return Err(RegexError::Syntax);
                }
                ranges.push((*a, *b));
            }
            _ => {
                add_atom(ranges, &first);
                ranges.push((DASH, DASH));
                add_atom(ranges, &second);
            }
        }
        Ok(false)
    }

    /// `ParseClassEscape` (or a plain class character).
    fn class_atom(&mut self) -> Rx<ClassAtom> {
        let c = self.cur().ok_or(RegexError::Syntax)?;
        if c != BACKSLASH {
            self.i += 1;
            return Ok(ClassAtom::Unit(c));
        }
        let n = self.at(self.i + 1).ok_or(RegexError::Syntax)?;
        if n == u16::from(b'b') {
            self.i += 2;
            return Ok(ClassAtom::Unit(0x08));
        }
        if let Some(set) = class_escape(n) {
            self.i += 2;
            return Ok(ClassAtom::Set(set));
        }
        self.i += 1;
        Ok(ClassAtom::Unit(self.character_escape(true)?))
    }
}

fn add_atom(ranges: &mut Vec<(u16, u16)>, atom: &ClassAtom) {
    match atom {
        ClassAtom::Unit(u) => ranges.push((*u, *u)),
        ClassAtom::Set(set) => ranges.extend_from_slice(set),
    }
}

/// `\d \D \s \S \w \W` as code unit ranges.
pub(super) fn class_escape(n: u16) -> Option<Vec<(u16, u16)>> {
    match u8::try_from(n).ok()? {
        b'd' => Some(vec![(0x30, 0x39)]),
        b'D' => Some(complement(vec![(0x30, 0x39)])),
        b's' => Some(space_ranges()),
        b'S' => Some(complement(space_ranges())),
        b'w' => Some(word_ranges()),
        b'W' => Some(complement(word_ranges())),
        _ => None,
    }
}
