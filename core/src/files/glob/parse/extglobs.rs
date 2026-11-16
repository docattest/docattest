//! parse.js: opening and closing an extglob (`!(…)`, `?(…)`, `+(…)`, `*(…)`, `@(…)`).

use super::{Counter, Extglob, Kind, Parser, Token, parse};
use crate::files::glob::constants::{GLOBSTAR, ONE_CHAR, STAR, escape_regex};
use crate::files::glob::extglob::analyze_repeated_extglob;
use crate::files::glob::units::{BACKSLASH, DOT, RPAREN, SLASH, STAR_CH, cat, is, slice, w};
use crate::files::glob::{U, UNDEFINED_TYPE};

impl Parser<'_> {
    pub(super) fn extglob_open(&mut self, kind: Kind, value: U) {
        let (open, close) = match kind {
            Kind::Negate => (w("(?:(?!(?:"), w(&format!(")){STAR})"))),
            Kind::Qmark => (w("(?:"), w(")?")),
            Kind::Plus => (w("(?:"), w(")+")),
            Kind::Star => (w("(?:"), w(")*")),
            _ => (w("(?:"), w(")")),
        };
        let entry = Extglob {
            kind,
            close,
            inner: U::new(),
            parens: self.parens,
            output: self.output.clone(),
            start_index: self.index,
            tokens_index: self.tokens.len(),
        };
        self.increment(Counter::Parens);
        let lead = if self.output.is_empty() { w(ONE_CHAR) } else { U::new() };
        self.push(Token::new(kind, value, Some(lead)));
        let paren = self.advance();
        let mut tok = Token::new(Kind::Paren, paren, Some(open));
        tok.extglob = true;
        self.push(tok);
        self.extglobs.push(entry);
    }

    pub(super) fn extglob_close(&mut self, token: Extglob, value: U) -> Result<(), String> {
        let start = usize::try_from(token.start_index).unwrap_or(0);
        let here = usize::try_from(self.index).unwrap_or(0);
        let literal = slice(self.input, start, here + 1).to_vec();
        let body = slice(self.input, start + 2, here).to_vec();
        let analysis = analyze_repeated_extglob(&body);

        if (token.kind == Kind::Plus || token.kind == Kind::Star) && analysis.risky {
            return self.close_risky_extglob(&token, value, literal, analysis.safe_output);
        }

        let output =
            if token.kind == Kind::Negate { self.negation_close(&token)? } else { token.close.clone() };
        let mut tok = Token::new(Kind::Paren, value, Some(output));
        tok.extglob = true;
        self.push(tok);
        self.decrement(Counter::Parens);
        Ok(())
    }

    /// A repeated extglob that could backtrack catastrophically: its tokens give way to the
    /// safe equivalent of its body, or else to the extglob as literal text.
    fn close_risky_extglob(
        &mut self,
        token: &Extglob,
        value: U,
        literal: U,
        safe_output: Option<U>,
    ) -> Result<(), String> {
        let safe_output = safe_output.map(|safe| {
            let lead = if token.output.is_empty() { w(ONE_CHAR) } else { U::new() };
            cat(&[&lead, &safe])
        });
        let Some(&open) = self.tokens.get(token.tokens_index) else {
            return Err(UNDEFINED_TYPE.to_string());
        };
        let open_output = safe_output.unwrap_or_else(|| escape_regex(&literal));
        {
            let o = self.tok_mut(open);
            o.kind = Kind::Text;
            o.value = literal;
            o.output = Some(open_output.clone());
        }
        let later: Vec<usize> = self.tokens.get(token.tokens_index + 1..).unwrap_or_default().to_vec();
        for id in later {
            let t = self.tok_mut(id);
            t.value = U::new();
            t.output = Some(U::new());
        }
        self.output = cat(&[&token.output, &open_output]);
        self.backtrack = true;
        let mut tok = Token::new(Kind::Paren, value, Some(U::new()));
        tok.extglob = true;
        self.push(tok);
        self.decrement(Counter::Parens);
        Ok(())
    }

    /// The close of `!(…)`, which depends on what the negation holds and what follows it.
    fn negation_close(&self, token: &Extglob) -> Result<U, String> {
        let mut output = token.close.clone();
        let extglob_star =
            if token.inner.len() > 1 && token.inner.contains(&SLASH) { w(GLOBSTAR) } else { w(STAR) };
        let rest = self.remaining();
        if !is(&extglob_star, STAR) || self.eos() || only_closing_parens(rest) {
            output = cat(&[&w(")$))"), &extglob_star]);
        }
        if token.inner.contains(&STAR_CH) && is_extension(rest) {
            let expression = parse(rest, false)?.output;
            output = cat(&[&w(")"), &expression, &w(")"), &extglob_star, &w(")")]);
        }
        Ok(output)
    }
}

/// `rest` is one `)` or more.
fn only_closing_parens(rest: &[u16]) -> bool {
    !rest.is_empty() && rest.iter().all(|&c| c == RPAREN)
}

/// `/^\.[^\\/.]+$/.test(rest)`.
fn is_extension(rest: &[u16]) -> bool {
    rest.len() >= 2
        && rest.first() == Some(&DOT)
        && rest.iter().skip(1).all(|&c| c != BACKSLASH && c != SLASH && c != DOT)
}
