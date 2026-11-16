//! The translator's grammar: escapes outside and inside a class, and backreferences.

use super::classes::{class_escape, class_source, literal};
use super::{Frag, K_MAX_CAPTURES, RegexError, Rx, Translator, hex_value, is_digit, is_octal};
use crate::files::glob::units::{BACKSLASH, LT};

impl Translator<'_> {
    /// An atom escape, at the backslash (something follows it).
    pub(super) fn atom_escape(&mut self) -> Rx<Frag> {
        let n = self.at(self.i + 1).unwrap_or(0);
        if (u16::from(b'1')..=u16::from(b'9')).contains(&n) {
            return Ok(self.decimal_escape(n));
        }
        if let Some(set) = class_escape(n) {
            self.i += 2;
            return Ok(Frag::atom(class_source(set)));
        }
        if n == u16::from(b'k') && self.has_named {
            self.i += 2;
            return self.named_backreference();
        }
        self.i += 1;
        let v = self.character_escape(false)?;
        Ok(Frag::atom(literal(v)))
    }

    /// `\1` to `\9…`: a backreference if there are that many groups, else (Annex B) the
    /// digits `8` and `9` as themselves or an octal escape.
    fn decimal_escape(&mut self, n: u16) -> Frag {
        if let Some((index, end)) = self.backreference_index(self.i + 1) {
            self.i = end;
            return self.backreference(&[index]);
        }
        if n == u16::from(b'8') || n == u16::from(b'9') {
            self.i += 2;
            return Frag::atom(literal(n));
        }
        self.i += 1;
        let v = self.octal();
        Frag::atom(literal(v))
    }

    /// `\k<name>`, just past the `k`.
    fn named_backreference(&mut self) -> Rx<Frag> {
        if self.cur() != Some(LT) {
            return Err(RegexError::Syntax);
        }
        self.i += 1;
        let name = self.group_name()?;
        if self.open.iter().any(|(_, open)| open.as_ref() == Some(&name)) {
            return Ok(Frag { s: String::new(), zero_width: true, bare: None });
        }
        let groups: Vec<usize> =
            self.known.iter().flatten().filter(|(known, _)| *known == name).map(|(_, g)| *g).collect();
        self.references.push(name);
        Ok(self.backreference(&groups))
    }

    /// `ParseBackReferenceIndex` at the first digit.
    fn backreference_index(&self, start: usize) -> Option<(usize, usize)> {
        let mut i = start;
        let mut value: u64 = 0;
        while is_digit(self.at(i)) {
            value = value * 10 + u64::from(self.at(i).unwrap_or(0x30)) - 0x30;
            if value > K_MAX_CAPTURES {
                return None;
            }
            i += 1;
        }
        let value = usize::try_from(value).ok()?;
        (value <= self.total_captures).then_some((value, i))
    }

    /// A backreference: JavaScript matches the empty string for a group that has not
    /// participated, and inside the group it names.
    fn backreference(&mut self, groups: &[usize]) -> Frag {
        self.backreferences.extend_from_slice(groups);
        if groups.iter().any(|g| self.open.iter().any(|(open, _)| open == g)) || groups.is_empty() {
            return Frag { s: String::new(), zero_width: true, bare: None };
        }
        let mut s = String::new();
        for g in groups.iter().rev() {
            s = format!("(?({g})\\{g}|{s})");
        }
        Frag::atom(s)
    }

    /// `ParseOctalLiteral` at the first digit.
    fn octal(&mut self) -> u16 {
        let digit = |c: Option<u16>| c.map_or(0, |c| c - u16::from(b'0'));
        let mut v = digit(self.cur());
        self.i += 1;
        if is_octal(self.cur()) {
            v = v * 8 + digit(self.cur());
            self.i += 1;
            if v < 32 && is_octal(self.cur()) {
                v = v * 8 + digit(self.cur());
                self.i += 1;
            }
        }
        v
    }

    /// `ParseCharacterEscape`, just past the backslash.
    pub(super) fn character_escape(&mut self, in_class: bool) -> Rx<u16> {
        let Some(c) = self.cur() else {
            return Err(RegexError::Syntax);
        };
        if let Some(v) = control_escape(c) {
            self.i += 1;
            return Ok(v);
        }
        if c == u16::from(b'c') {
            return Ok(self.control_letter(in_class));
        }
        if c == u16::from(b'0') && !is_digit(self.at(self.i + 1)) {
            self.i += 1;
            return Ok(0);
        }
        if (u16::from(b'0')..=u16::from(b'7')).contains(&c) {
            return Ok(self.octal());
        }
        if let Some(digits) = hex_digits(c) {
            return Ok(self.hex_escape(c, digits));
        }
        if c == u16::from(b'k') && self.has_named {
            return Err(RegexError::Syntax);
        }
        self.i += 1;
        Ok(c)
    }

    /// `\c` and a control letter (in a class, also a digit or `_`), at the `c`.
    fn control_letter(&mut self, in_class: bool) -> u16 {
        if let Some(n) = self.at(self.i + 1) {
            let letter = n & !0x20;
            let control = (u16::from(b'A')..=u16::from(b'Z')).contains(&letter)
                || (in_class && (is_digit(Some(n)) || n == u16::from(b'_')));
            if control {
                self.i += 2;
                return n & 0x1F;
            }
        }
        // The backslash is a literal; the `c` is read again as the next atom.
        BACKSLASH
    }

    /// `\xXX` or `\uXXXX`, at the letter; without all its digits, the letter itself.
    fn hex_escape(&mut self, c: u16, digits: usize) -> u16 {
        let mut v: u32 = 0;
        for k in 0..digits {
            match self.at(self.i + 1 + k).and_then(hex_value) {
                Some(d) => v = v * 16 + d,
                None => {
                    self.i += 1;
                    return c;
                }
            }
        }
        self.i += 1 + digits;
        u16::try_from(v).unwrap_or(c)
    }
}

/// `\f \n \r \t \v`.
fn control_escape(c: u16) -> Option<u16> {
    const ESCAPES: [(u8, u16); 5] = [(b'f', 0x0C), (b'n', 0x0A), (b'r', 0x0D), (b't', 0x09), (b'v', 0x0B)];
    let c = u8::try_from(c).ok()?;
    ESCAPES.iter().find(|&&(letter, _)| letter == c).map(|&(_, v)| v)
}

/// How many hex digits follow `x` and `u`.
fn hex_digits(c: u16) -> Option<usize> {
    if c == u16::from(b'x') {
        Some(2)
    } else if c == u16::from(b'u') {
        Some(4)
    } else {
        None
    }
}
