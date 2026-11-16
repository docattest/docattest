//! parse.js: what the main loop does with the characters that open and close a group,
//! `(` `)`, `[` `]` and `{` `}`, and with the `,` that separates a brace's alternatives.

use super::{Counter, Kind, Parser, Token};
use crate::files::glob::U;
use crate::files::glob::constants::{escape_regex, has_regex_chars};
use crate::files::glob::translate::{RegexError, translate};
use crate::files::glob::units::{BACKSLASH, CARET, DASH, RBRACKET, SLASH, cat, drop_last, ext, slice, w};

impl Parser<'_> {
    pub(super) fn open_paren(&mut self, value: U) {
        self.increment(Counter::Parens);
        self.push(Token::new(Kind::Paren, value, None));
    }

    pub(super) fn close_paren(&mut self, value: U) -> Result<(), String> {
        let closes_extglob = self.extglobs.last().is_some_and(|e| self.parens == e.parens + 1);
        if closes_extglob {
            if let Some(token) = self.extglobs.pop() {
                self.extglob_close(token, value)?;
            }
            return Ok(());
        }
        let output = if self.parens != 0 { w(")") } else { w(r"\)") };
        self.push(Token::new(Kind::Paren, value, Some(output)));
        self.decrement(Counter::Parens);
        Ok(())
    }

    pub(super) fn open_bracket(&mut self, mut value: U) {
        if !self.remaining().contains(&RBRACKET) {
            value.insert(0, BACKSLASH);
        } else {
            self.increment(Counter::Brackets);
        }
        self.push(Token::new(Kind::Bracket, value, None));
    }

    pub(super) fn close_bracket(&mut self, mut value: U) {
        let prev = self.prev;
        if self.tok(prev).kind == Kind::Bracket && self.tok(prev).value.len() == 1 {
            self.push(Token::new(Kind::Text, value, Some(w(r"\]"))));
            return;
        }
        if self.brackets == 0 {
            self.push(Token::new(Kind::Text, value, Some(w(r"\]"))));
            return;
        }
        self.decrement(Counter::Brackets);
        let pv = self.tok(prev).value.clone();
        let prev_value = slice(&pv, 1, pv.len());
        if !self.tok(prev).posix && prev_value.first() == Some(&CARET) && !prev_value.contains(&SLASH) {
            value.insert(0, SLASH);
        }
        self.tok_mut(prev).value.extend_from_slice(&value);
        self.append(&value);
        if has_regex_chars(prev_value) {
            return;
        }
        let full = self.tok(prev).value.clone();
        let escaped = escape_regex(&full);
        drop_last(&mut self.output, full.len());
        let both = cat(&[&w("(?:"), &escaped, &w("|"), &full, &w(")")]);
        self.output.extend_from_slice(&both);
        self.tok_mut(prev).value = both;
    }

    pub(super) fn open_brace(&mut self, value: U) {
        self.increment(Counter::Braces);
        let mut open = Token::new(Kind::Brace, value, Some(w("(")));
        open.output_index = self.output.len();
        open.tokens_index = self.tokens.len();
        if let Some(id) = self.push(open) {
            self.braces.push(id);
        }
    }

    pub(super) fn close_brace(&mut self, mut value: U) {
        let Some(&brace) = self.braces.last() else {
            let output = value.clone();
            self.push(Token::new(Kind::Text, value, Some(output)));
            return;
        };
        let mut output = w(")");
        if self.tok(brace).dots {
            let mut range: Vec<U> = Vec::new();
            while let Some(id) = self.tokens.pop() {
                if self.tok(id).kind == Kind::Brace {
                    break;
                }
                if self.tok(id).kind != Kind::Dots {
                    range.push(self.tok(id).value.clone());
                }
            }
            range.reverse();
            output = expand_range(range);
            self.backtrack = true;
        }
        if !self.tok(brace).comma && !self.tok(brace).dots {
            let out = slice(&self.output, 0, self.tok(brace).output_index).to_vec();
            let from = self.tok(brace).tokens_index;
            {
                let b = self.tok_mut(brace);
                b.value = w(r"\{");
                b.output = Some(w(r"\{"));
            }
            value = w(r"\}");
            output = w(r"\}");
            let mut rebuilt = out;
            for &id in self.tokens.get(from..).unwrap_or_default() {
                rebuilt.extend_from_slice(self.tok(id).output_or_value());
            }
            self.output = rebuilt;
        }
        self.push(Token::new(Kind::Brace, value, Some(output)));
        self.decrement(Counter::Braces);
        self.braces.pop();
    }

    pub(super) fn comma(&mut self, value: U) {
        let mut output = value.clone();
        if let Some(&brace) = self.braces.last()
            && self.stack.last() == Some(&Counter::Braces)
        {
            self.tok_mut(brace).comma = true;
            output = w("|");
        }
        self.push(Token::new(Kind::Comma, value, Some(output)));
    }
}

/// `expandRange(args, opts)`.
fn expand_range(mut args: Vec<U>) -> U {
    args.sort();
    let mut value = w("[");
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            value.push(DASH);
        }
        value.extend_from_slice(a);
    }
    value.push(RBRACKET);
    if !matches!(translate(&value), Err(RegexError::Syntax)) {
        return value;
    }
    let mut out = U::new();
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            ext(&mut out, "..");
        }
        out.extend(escape_regex(a));
    }
    out
}
