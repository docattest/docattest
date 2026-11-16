//! parse.js: what the main loop does with `"`, `|`, `/`, `.`, `?`, `!`, `+`, `@` and any
//! character without a meaning of its own.

use super::{BOS, Kind, Parser, Token};
use crate::files::glob::U;
use crate::files::glob::constants::{DOT_LITERAL, PLUS_LITERAL, QMARK, SLASH_LITERAL};
use crate::files::glob::units::{
    BACKSLASH, BANG, CARET, COLON, DOLLAR, EQUALS, GT, LPAREN, LT, QMARK_CH, is, is_word_unit, slice, w,
};

/// What may follow `(?` to make a group an assertion or a named group.
const ASSERTION_CHARS: [u16; 4] = [BANG, EQUALS, LT, COLON];

impl Parser<'_> {
    pub(super) fn toggle_quotes(&mut self, _value: U) {
        self.quotes = if self.quotes == 1 { 0 } else { 1 };
    }

    pub(super) fn pipe(&mut self, value: U) {
        self.push(Token::text(value));
    }

    pub(super) fn slash(&mut self, value: U) {
        if self.tok(self.prev).kind == Kind::Dot && self.index == self.start + 1 {
            self.start = self.index + 1;
            self.output.clear();
            self.tokens.pop();
            self.prev = BOS;
            return;
        }
        self.push(Token::new(Kind::Slash, value, Some(w(SLASH_LITERAL))));
    }

    pub(super) fn dot(&mut self, value: U) {
        let prev = self.prev;
        if self.brace_count > 0 && self.tok(prev).kind == Kind::Dot {
            let p = self.tok_mut(prev);
            if is(&p.value, ".") {
                p.output = Some(w(DOT_LITERAL));
            }
            p.kind = Kind::Dots;
            p.output.get_or_insert_with(|| w("undefined")).extend_from_slice(&value);
            p.value.extend_from_slice(&value);
            if let Some(&brace) = self.braces.last() {
                self.tok_mut(brace).dots = true;
            }
            return;
        }
        let kind = self.tok(prev).kind;
        if self.brace_count + self.parens == 0 && kind != Kind::Bos && kind != Kind::Slash {
            self.push(Token::new(Kind::Text, value, Some(w(DOT_LITERAL))));
            return;
        }
        self.push(Token::new(Kind::Dot, value, Some(w(DOT_LITERAL))));
    }

    pub(super) fn qmark(&mut self, value: U) {
        let is_group = self.prev_value_is("(");
        if !is_group && self.peek(1) == Some(LPAREN) && self.peek(2) != Some(QMARK_CH) {
            self.extglob_open(Kind::Qmark, value);
            return;
        }
        if self.tok(self.prev).kind == Kind::Paren {
            let next = self.peek(1);
            let assertion_char = next.is_some_and(|n| ASSERTION_CHARS.contains(&n));
            let output = if (is_group && !assertion_char)
                || (next == Some(LT) && !lookbehind_or_name(self.remaining()))
            {
                w(r"\?")
            } else {
                value.clone()
            };
            self.push(Token::new(Kind::Text, value, Some(output)));
            return;
        }
        self.push(Token::new(Kind::Qmark, value, Some(w(QMARK))));
    }

    pub(super) fn bang(&mut self, value: U) {
        if self.peek(1) == Some(LPAREN)
            && (self.peek(2) != Some(QMARK_CH) || !self.peek(3).is_some_and(|n| ASSERTION_CHARS.contains(&n)))
        {
            self.extglob_open(Kind::Negate, value);
            return;
        }
        if self.index == 0 {
            self.negate();
            return;
        }
        self.literal(BANG, value);
    }

    pub(super) fn plus(&mut self, value: U) {
        if self.peek(1) == Some(LPAREN) && self.peek(2) != Some(QMARK_CH) {
            self.extglob_open(Kind::Plus, value);
            return;
        }
        if self.prev_value_is("(") {
            self.push(Token::new(Kind::Plus, value, Some(w(PLUS_LITERAL))));
            return;
        }
        let kind = self.tok(self.prev).kind;
        if matches!(kind, Kind::Bracket | Kind::Paren | Kind::Brace) || self.parens > 0 {
            self.push(Token::new(Kind::Plus, value, None));
            return;
        }
        self.push(Token::new(Kind::Plus, w(PLUS_LITERAL), None));
    }

    pub(super) fn at_sign(&mut self, value: U) {
        if self.peek(1) == Some(LPAREN) && self.peek(2) != Some(QMARK_CH) {
            let mut tok = Token::new(Kind::At, value, Some(U::new()));
            tok.extglob = true;
            self.push(tok);
            return;
        }
        self.push(Token::text(value));
    }

    /// Any other character, with the run of plain characters after it, as text.
    pub(super) fn literal(&mut self, c: u16, mut value: U) {
        if c == DOLLAR || c == CARET {
            value.insert(0, BACKSLASH);
        }
        // `REGEX_NON_SPECIAL_CHARS`
        let run = self
            .remaining()
            .iter()
            .take_while(|&&x| !"@![].,$*+?^{}()|\\/".encode_utf16().any(|s| s == x))
            .count();
        if run > 0 {
            value.extend_from_slice(slice(self.remaining(), 0, run));
            self.index += i64::try_from(run).unwrap_or(0);
        }
        self.push(Token::text(value));
    }
}

/// `/<([!=]|\w+>)/.test(rest)`.
fn lookbehind_or_name(rest: &[u16]) -> bool {
    rest.iter().enumerate().any(|(i, &c)| {
        if c != LT {
            return false;
        }
        match rest.get(i + 1) {
            Some(&n) if n == BANG || n == EQUALS => true,
            _ => {
                let tail = slice(rest, i + 1, rest.len());
                let word = tail.iter().take_while(|&&x| is_word_unit(x)).count();
                word > 0 && tail.get(word) == Some(&GT)
            }
        }
    })
}
