//! The translator's grammar: disjunctions, alternatives, terms and quantifiers.

use super::classes::{WORD_CLASS, class_source, complement, literal};
use super::{Disjunction, Frag, K_INFINITY, MAX_DEPTH, RegexError, Rx, Term, Translator, is_digit};
use crate::files::glob::units::{
    BACKSLASH, CARET, COMMA, DOLLAR, DOT, LBRACE, LBRACKET, LPAREN, PIPE, PLUS_CH, QMARK_CH, RBRACE, RPAREN,
    STAR_CH,
};

impl Translator<'_> {
    pub(super) fn run(&mut self) -> Rx<String> {
        let d = self.disjunction()?;
        if self.i < self.s.len() {
            // An unmatched `)`.
            return Err(RegexError::Syntax);
        }
        for r in &self.references {
            if !self.names.iter().any(|(n, _)| n == r) {
                return Err(RegexError::Syntax);
            }
        }
        Ok(d.s)
    }

    pub(super) fn disjunction(&mut self) -> Rx<Disjunction> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(RegexError::Unsupported);
        }
        let mut parts = Vec::new();
        let mut zero_width = true;
        loop {
            let (s, zw) = self.alternative()?;
            zero_width &= zw;
            parts.push(s);
            if self.cur() == Some(PIPE) {
                self.i += 1;
                let started = self.captures_started;
                if let Some(state) = self.states.last_mut() {
                    if state.interval.1 != 0 {
                        state.interval.1 = started;
                    } else {
                        state.interval = (state.capture_index, started);
                    }
                }
                continue;
            }
            break;
        }
        self.depth -= 1;
        Ok(Disjunction { s: parts.join("|"), zero_width, parts })
    }

    fn alternative(&mut self) -> Rx<(String, bool)> {
        let mut s = String::new();
        let mut zero_width = true;
        while let Some(c) = self.cur() {
            if c == PIPE || c == RPAREN {
                break;
            }
            let before = self.captures_started;
            match self.term(c)? {
                Term::Assertion(a) => s.push_str(&a),
                Term::Atom(frag) => {
                    let (frag, repeats) = self.quantifier(frag)?;
                    if repeats && self.captures_started > before {
                        self.repeated.push((before + 1, self.captures_started));
                    }
                    zero_width &= frag.zero_width;
                    s.push_str(&frag.s);
                }
            }
        }
        Ok((s, zero_width))
    }

    fn term(&mut self, c: u16) -> Rx<Term> {
        match c {
            CARET => {
                self.i += 1;
                Ok(Term::Assertion("^".into()))
            }
            DOLLAR => {
                self.i += 1;
                Ok(Term::Assertion("$".into()))
            }
            BACKSLASH => self.escape_term(),
            LPAREN => self.group(),
            LBRACKET => Ok(Term::Atom(self.class()?)),
            DOT => {
                self.i += 1;
                let dot = complement(vec![(0x0A, 0x0A), (0x0D, 0x0D), (0x2028, 0x2029)]);
                Ok(Term::Atom(Frag::atom(class_source(dot))))
            }
            STAR_CH | PLUS_CH | QMARK_CH => Err(RegexError::Syntax),
            _ => self.literal_term(c),
        }
    }

    /// At a backslash: a word boundary assertion or an atom escape.
    fn escape_term(&mut self) -> Rx<Term> {
        match self.at(self.i + 1) {
            None => Err(RegexError::Syntax),
            Some(n) if n == u16::from(b'b') => {
                self.i += 2;
                Ok(Term::Assertion(format!(
                    "(?:(?<={WORD_CLASS})(?!{WORD_CLASS})|(?<!{WORD_CLASS})(?={WORD_CLASS}))"
                )))
            }
            Some(n) if n == u16::from(b'B') => {
                self.i += 2;
                Ok(Term::Assertion(format!(
                    "(?:(?<={WORD_CLASS})(?={WORD_CLASS})|(?<!{WORD_CLASS})(?!{WORD_CLASS}))"
                )))
            }
            Some(_) => Ok(Term::Atom(self.atom_escape()?)),
        }
    }

    /// A character that stands for itself; a `{` that opens a valid interval has nothing to
    /// repeat.
    fn literal_term(&mut self, c: u16) -> Rx<Term> {
        if c == LBRACE && self.interval(self.i).is_some() {
            return Err(RegexError::Syntax);
        }
        self.i += 1;
        Ok(Term::Atom(Frag::atom(literal(c))))
    }

    /// `ParseIntervalQuantifier` at `{`: `(min, max, end)`, `max` `None` for unbounded.
    fn interval(&self, start: usize) -> Option<(u64, Option<u64>, usize)> {
        let mut i = start + 1;
        if !is_digit(self.at(i)) {
            return None;
        }
        let min = self.decimal(&mut i);
        let max = match self.at(i) {
            Some(RBRACE) => Some(min),
            Some(COMMA) => {
                i += 1;
                if self.at(i) == Some(RBRACE) {
                    None
                } else {
                    if !is_digit(self.at(i)) {
                        return None;
                    }
                    let max = self.decimal(&mut i);
                    if self.at(i) != Some(RBRACE) {
                        return None;
                    }
                    Some(max)
                }
            }
            _ => return None,
        };
        let max = max.filter(|&m| m < K_INFINITY);
        Some((min, max, i + 1))
    }

    /// The digits at `i`, read past, as a number that stops growing at `K_INFINITY`.
    fn decimal(&self, i: &mut usize) -> u64 {
        let mut v: u64 = 0;
        while is_digit(self.at(*i)) {
            let d = u64::from(self.at(*i).unwrap_or(0x30)) - 0x30;
            v = (v * 10 + d).min(K_INFINITY);
            *i += 1;
        }
        v
    }

    /// Applies a following quantifier, if any; also says whether the atom may repeat.
    fn quantifier(&mut self, frag: Frag) -> Rx<(Frag, bool)> {
        let Some((min, max)) = self.read_quantifier() else {
            return Ok((self.unquantified(frag), false));
        };
        let lazy = self.cur() == Some(QMARK_CH);
        if lazy {
            self.i += 1;
        }
        if max.is_some_and(|m| min > m) {
            return Err(RegexError::Syntax);
        }
        if frag.zero_width || max == Some(0) {
            // JavaScript ends a loop at an iteration that matches nothing once the minimum
            // is met, so the atom runs `min` times at one position: once or never.
            let s = if min == 0 { format!("(?:(?!){}|)", frag.s) } else { frag.s };
            return Ok((Frag { s, zero_width: true, bare: None }, false));
        }
        let mut s = frag.s;
        s.push_str(&quantifier_source(min, max));
        if lazy {
            s.push('?');
        }
        let repeats = max.is_none_or(|m| m > 1);
        Ok((Frag { s, zero_width: false, bare: None }, repeats))
    }

    /// Reads a quantifier's bounds, `(min, max)` with `max` `None` for unbounded; `None`
    /// when no quantifier follows.
    fn read_quantifier(&mut self) -> Option<(u64, Option<u64>)> {
        let bounds = match self.cur() {
            Some(STAR_CH) => (0, None),
            Some(PLUS_CH) => (1, None),
            Some(QMARK_CH) => (0, Some(1)),
            Some(LBRACE) => {
                let (min, max, end) = self.interval(self.i)?;
                self.i = end;
                return Some((min, max));
            }
            _ => return None,
        };
        self.i += 1;
        Some(bounds)
    }

    fn unquantified(&self, frag: Frag) -> Frag {
        match frag.bare {
            Some(bare) => Frag { s: bare, zero_width: frag.zero_width, bare: None },
            None => frag,
        }
    }
}

/// A quantifier as fancy_regex writes it.
fn quantifier_source(min: u64, max: Option<u64>) -> String {
    match (min, max) {
        (0, None) => "*".into(),
        (1, None) => "+".into(),
        (0, Some(1)) => "?".into(),
        (m, None) => format!("{{{m},}}"),
        (m, Some(n)) if m == n => format!("{{{m}}}"),
        (m, Some(n)) => format!("{{{m},{n}}}"),
    }
}
