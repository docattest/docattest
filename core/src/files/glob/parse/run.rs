//! parse.js: the main loop, one character at a time.

use super::{BOS, Counter, Kind, Parser, Token};
use crate::files::glob::constants::{ONE_CHAR, SLASH_LITERAL, escape_last, escape_regex, posix_class};
use crate::files::glob::units::{
    AT, BACKSLASH, BANG, COLON, COMMA, DASH, DOT, LBRACE, LBRACKET, LPAREN, PIPE, PLUS_CH, QMARK_CH, QUOTE,
    RBRACE, RBRACKET, RPAREN, SEMI, SLASH, STAR_CH, cat, slice, w,
};
use crate::files::glob::{HANGS, U};

/// What the loop does with one special character.
type Handler<'a> = fn(&mut Parser<'a>, U);

impl<'a> Parser<'a> {
    /// What the loop does with each special character outside a class or quotes; `)` may
    /// fail and is handled apart, and any other character is a literal.
    const HANDLERS: [(u16, Handler<'a>); 15] = [
        (QUOTE, Parser::toggle_quotes),
        (LPAREN, Parser::open_paren),
        (LBRACKET, Parser::open_bracket),
        (RBRACKET, Parser::close_bracket),
        (LBRACE, Parser::open_brace),
        (RBRACE, Parser::close_brace),
        (PIPE, Parser::pipe),
        (COMMA, Parser::comma),
        (SLASH, Parser::slash),
        (DOT, Parser::dot),
        (QMARK_CH, Parser::qmark),
        (BANG, Parser::bang),
        (PLUS_CH, Parser::plus),
        (AT, Parser::at_sign),
        (STAR_CH, Parser::star),
    ];

    pub(super) fn run(&mut self) -> Result<(), String> {
        while !self.eos() {
            if self.index >= self.len() {
                // Past the end `eos()` can never be true again: picomatch spins forever.
                return Err(HANGS.to_string());
            }
            let value = self.advance();
            self.step(value)?;
        }
        self.escape_unclosed();
        self.finish();
        Ok(())
    }

    /// One turn of the loop, for the character just advanced over.
    fn step(&mut self, value: U) -> Result<(), String> {
        if value == [0] {
            return Ok(());
        }
        let value = if value == [BACKSLASH] {
            match self.escaped(value) {
                Some(pair) => pair,
                None => return Ok(()),
            }
        } else {
            value
        };
        if self.in_class(&value) {
            self.class_character(value);
            return Ok(());
        }
        if self.quotes == 1 && value != [QUOTE] {
            self.quoted(&value);
            return Ok(());
        }
        let Some(&c) = value.first() else {
            // Only reachable past the end, which the loop head rejects.
            return Err(HANGS.to_string());
        };
        self.special(c, value)
    }

    /// Escaped characters: `None` when the escape is consumed here, else the escaped pair,
    /// which a character class takes.
    fn escaped(&mut self, mut value: U) -> Option<U> {
        let next = self.peek(1);
        if next == Some(SLASH) || next == Some(DOT) || next == Some(SEMI) {
            return None;
        }
        if next.is_none() {
            value.push(BACKSLASH);
            self.push(Token::text(value));
            return None;
        }
        let run = self.remaining().iter().take_while(|&&c| c == BACKSLASH).count();
        if run > 2 {
            self.index += i64::try_from(run).unwrap_or(i64::MAX / 2);
            if run % 2 != 0 {
                value.push(BACKSLASH);
            }
        }
        let next = self.advance();
        value.extend(next);
        if self.brackets == 0 {
            self.push(Token::text(value));
            return None;
        }
        Some(value)
    }

    /// Inside a character class, where all but its closing `]` is the class's.
    fn in_class(&self, value: &[u16]) -> bool {
        self.brackets > 0 && (value != [RBRACKET] || self.prev_value_is("[") || self.prev_value_is("[^"))
    }

    fn class_character(&mut self, mut value: U) {
        if value == [COLON] && self.close_posix_class() {
            return;
        }
        if (value == [LBRACKET] && self.peek(1) != Some(COLON))
            || (value == [DASH] && self.peek(1) == Some(RBRACKET))
        {
            value.insert(0, BACKSLASH);
        }
        if value == [RBRACKET] && (self.prev_value_is("[") || self.prev_value_is("[^")) {
            value.insert(0, BACKSLASH);
        }
        self.tok_mut(self.prev).value.extend_from_slice(&value);
        self.append(&value);
    }

    /// The `:` that may end `[:name:]`: a known class becomes its characters, and the `:`
    /// is consumed (true).
    fn close_posix_class(&mut self) -> bool {
        let pv = self.tok(self.prev).value.clone();
        let inner = slice(&pv, 1, pv.len());
        if !inner.contains(&LBRACKET) {
            return false;
        }
        self.tok_mut(self.prev).posix = true;
        if !inner.contains(&COLON) {
            return false;
        }
        let idx = pv.iter().rposition(|&c| c == LBRACKET).unwrap_or(0);
        let pre = slice(&pv, 0, idx);
        let rest = slice(&pv, idx + 2, pv.len());
        let Some(posix) = posix_class(rest) else {
            return false;
        };
        let replaced = cat(&[pre, &w(posix)]);
        self.tok_mut(self.prev).value = replaced;
        self.backtrack = true;
        self.advance();
        let at_one = self.tokens.iter().position(|&t| t == self.prev);
        if self.tok(BOS).emitted().is_empty() && at_one == Some(1) {
            self.tok_mut(BOS).output = Some(w(ONE_CHAR));
        }
        true
    }

    /// Inside double quotes.
    fn quoted(&mut self, value: &[u16]) {
        let escaped = escape_regex(value);
        self.tok_mut(self.prev).value.extend_from_slice(&escaped);
        self.append(&escaped);
    }

    fn special(&mut self, c: u16, value: U) -> Result<(), String> {
        if c == RPAREN {
            return self.close_paren(value);
        }
        match Self::HANDLERS.iter().find(|&&(special, _)| special == c) {
            Some(&(_, handle)) => handle(self, value),
            None => self.literal(c, value),
        }
        Ok(())
    }

    /// At the end, every group still open is escaped, innermost first in each kind.
    fn escape_unclosed(&mut self) {
        while self.brackets > 0 {
            self.output = escape_last(&self.output, LBRACKET);
            self.decrement(Counter::Brackets);
        }
        while self.parens > 0 {
            self.output = escape_last(&self.output, LPAREN);
            self.decrement(Counter::Parens);
        }
        while self.brace_count > 0 {
            self.output = escape_last(&self.output, LBRACE);
            self.decrement(Counter::Braces);
        }
    }

    fn finish(&mut self) {
        let last = self.tok(self.prev).kind;
        if last == Kind::Star || last == Kind::Bracket {
            let maybe = format!("{SLASH_LITERAL}?");
            self.push(Token::new(Kind::MaybeSlash, U::new(), Some(w(&maybe))));
        }
        if self.backtrack {
            let mut output = U::new();
            for &id in &self.tokens {
                output.extend_from_slice(self.tok(id).emitted());
            }
            self.output = output;
        }
    }
}
