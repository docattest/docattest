//! The translator's grammar: groups, lookarounds and group names.

use super::{
    Disjunction, Frag, GroupState, RegexError, Rx, Term, Translator, hex_value, is_id_continue, is_id_start,
};
use crate::files::glob::units::{BACKSLASH, BANG, COLON, EQUALS, GT, LBRACE, LT, QMARK_CH, RBRACE, RPAREN};

/// What the opening of a group makes it.
enum GroupKind {
    NonCapture,
    Ahead(&'static str),
    Behind(&'static str),
    Capture(Option<Vec<u32>>),
}

impl Translator<'_> {
    pub(super) fn group(&mut self) -> Rx<Term> {
        let kind = self.group_kind()?;
        let parent = self.states.last().copied().unwrap_or(GroupState { capture_index: 0, interval: (0, 0) });
        let capture = self.open_capture(&kind);
        self.states.push(GroupState {
            capture_index: capture.unwrap_or(parent.capture_index),
            interval: parent.interval,
        });
        let inner = self.disjunction()?;
        let state = self.states.pop().unwrap_or(parent);
        if let Some(index) = capture {
            self.open.pop();
            if let GroupKind::Capture(Some(name)) = &kind {
                self.check_name_reusable(name, state.interval)?;
                self.names.push((name.clone(), index));
            }
        }
        if self.cur() != Some(RPAREN) {
            return Err(RegexError::Syntax);
        }
        self.i += 1;
        Ok(group_term(kind, &inner))
    }

    /// Reads the opening of a group, `(`, `(?:`, `(?=`, `(?!`, `(?<=`, `(?<!` or `(?<name>`.
    fn group_kind(&mut self) -> Rx<GroupKind> {
        if self.at(self.i + 1) != Some(QMARK_CH) {
            self.i += 1;
            return Ok(GroupKind::Capture(None));
        }
        let kind = match self.at(self.i + 2) {
            Some(COLON) => GroupKind::NonCapture,
            Some(EQUALS) => GroupKind::Ahead("(?="),
            Some(BANG) => GroupKind::Ahead("(?!"),
            Some(LT) => return self.behind_or_named(),
            // Modifiers such as `(?i:…)` are valid in current V8; picomatch never emits
            // them (it escapes a `?` after `(` unless one of `!=<:` follows).
            Some(c) if "ims-".encode_utf16().any(|m| m == c) => {
                return Err(RegexError::Unsupported);
            }
            _ => return Err(RegexError::Syntax),
        };
        self.i += 3;
        Ok(kind)
    }

    /// After `(?<`: a lookbehind or a named group.
    fn behind_or_named(&mut self) -> Rx<GroupKind> {
        match self.at(self.i + 3) {
            Some(EQUALS) => {
                self.i += 4;
                Ok(GroupKind::Behind("(?<="))
            }
            Some(BANG) => {
                self.i += 4;
                Ok(GroupKind::Behind("(?<!"))
            }
            _ => {
                self.i += 3;
                Ok(GroupKind::Capture(Some(self.group_name()?)))
            }
        }
    }

    /// Numbers a capturing group as it opens.
    fn open_capture(&mut self, kind: &GroupKind) -> Option<usize> {
        let GroupKind::Capture(name) = kind else {
            return None;
        };
        self.captures_started += 1;
        let index = self.captures_started;
        self.open.push((index, name.clone()));
        Some(index)
    }

    /// A name may be reused only by groups in other alternatives (V8's duplicate check).
    fn check_name_reusable(&self, name: &[u32], (start, end): (usize, usize)) -> Rx<()> {
        for (other, e) in &self.names {
            if other == name && !(start < *e && *e <= end) {
                return Err(RegexError::Syntax);
            }
        }
        Ok(())
    }

    /// `ParseCaptureGroupName`, just past the `<`.
    pub(super) fn group_name(&mut self) -> Rx<Vec<u32>> {
        let mut name: Vec<u32> = Vec::new();
        loop {
            let c = self.cur().ok_or(RegexError::Syntax)?;
            if c == GT {
                if name.is_empty() {
                    return Err(RegexError::Syntax);
                }
                self.i += 1;
                return Ok(name);
            }
            let cp = if c == BACKSLASH { self.name_escape()? } else { self.name_unit(c) };
            let ok = if name.is_empty() { is_id_start(cp) } else { is_id_continue(cp) };
            if !ok {
                return Err(RegexError::Syntax);
            }
            name.push(cp);
        }
    }

    /// `\u…` in a group name, at the backslash; an escaped surrogate pair is one code point.
    fn name_escape(&mut self) -> Rx<u32> {
        self.i += 1;
        if self.cur() != Some(u16::from(b'u')) {
            return Err(RegexError::Syntax);
        }
        self.i += 1;
        let first = self.unicode_escape()?;
        if !(0xD800..=0xDBFF).contains(&first)
            || self.cur() != Some(BACKSLASH)
            || self.at(self.i + 1) != Some(u16::from(b'u'))
        {
            return Ok(first);
        }
        let save = self.i;
        self.i += 2;
        match self.unicode_escape() {
            Ok(second) if (0xDC00..=0xDFFF).contains(&second) => {
                Ok(0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00))
            }
            _ => {
                self.i = save;
                Ok(first)
            }
        }
    }

    /// A plain character of a group name; a surrogate pair is one code point.
    fn name_unit(&mut self, c: u16) -> u32 {
        self.i += 1;
        match self.cur() {
            Some(n) if (0xD800..=0xDBFF).contains(&c) && (0xDC00..=0xDFFF).contains(&n) => {
                self.i += 1;
                0x10000 + ((u32::from(c) - 0xD800) << 10) + (u32::from(n) - 0xDC00)
            }
            _ => u32::from(c),
        }
    }

    /// `\u` in a group name, just past the `u`: `XXXX` or `{X…}`.
    fn unicode_escape(&mut self) -> Rx<u32> {
        if self.cur() == Some(LBRACE) {
            let mut i = self.i + 1;
            let mut v: u32 = 0;
            let mut digits = 0;
            while let Some(d) = self.at(i).and_then(hex_value) {
                v = v.saturating_mul(16).saturating_add(d);
                digits += 1;
                i += 1;
            }
            if digits == 0 || self.at(i) != Some(RBRACE) || v > 0x10_FFFF {
                return Err(RegexError::Syntax);
            }
            self.i = i + 1;
            return Ok(v);
        }
        let mut v = 0;
        for k in 0..4 {
            let d = self.at(self.i + k).and_then(hex_value).ok_or(RegexError::Syntax)?;
            v = v * 16 + d;
        }
        self.i += 4;
        Ok(v)
    }
}

/// The term a group makes of its content.
fn group_term(kind: GroupKind, inner: &Disjunction) -> Term {
    match kind {
        GroupKind::NonCapture => Term::Atom(Frag {
            s: format!("(?:{})", inner.s),
            zero_width: inner.zero_width,
            bare: (inner.parts.len() == 1).then(|| inner.s.clone()),
        }),
        GroupKind::Ahead(open) => {
            Term::Atom(Frag { s: format!("{open}{})", inner.s), zero_width: true, bare: None })
        }
        // fancy_regex needs a constant length behind; alternatives of different
        // constant lengths still work when each gets its own lookbehind.
        GroupKind::Behind(open) if inner.parts.len() > 1 => {
            let each: Vec<String> = inner.parts.iter().map(|part| format!("{open}{part})")).collect();
            Term::Assertion(if open == "(?<=" { format!("(?:{})", each.join("|")) } else { each.concat() })
        }
        GroupKind::Behind(open) => Term::Assertion(format!("{open}{})", inner.s)),
        GroupKind::Capture(_) => {
            Term::Atom(Frag { s: format!("({})", inner.s), zero_width: inner.zero_width, bare: None })
        }
    }
}
