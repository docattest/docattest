//! parse.js: what the main loop does with `*`: a star, a globstar (`**`) in its several
//! places, or the start of a `*(…)` extglob.

use super::{BOS, Kind, Parser, Token};
use crate::files::glob::U;
use crate::files::glob::constants::{GLOBSTAR, NO_DOT_SLASH, NO_DOTS_SLASH, ONE_CHAR, SLASH_LITERAL, STAR};
use crate::files::glob::units::{LPAREN, QMARK_CH, RPAREN, SLASH, STAR_CH, cat, drop_last, ext, slice, w};

impl<'a> Parser<'a> {
    pub(super) fn star(&mut self, value: U) {
        let prev = self.prev;
        if self.tok(prev).kind == Kind::Globstar || self.tok(prev).star {
            let p = self.tok_mut(prev);
            p.kind = Kind::Star;
            p.star = true;
            p.value.extend_from_slice(&value);
            p.output = Some(w(STAR));
            self.backtrack = true;
            return;
        }
        let rest = self.remaining();
        // `/^\([^?]/`
        if rest.first() == Some(&LPAREN) && rest.get(1).is_some_and(|&c| c != QMARK_CH) {
            self.extglob_open(Kind::Star, value);
            return;
        }
        if self.tok(prev).kind == Kind::Star {
            self.second_star(value);
            return;
        }
        self.first_star(value);
    }

    /// A star that does not follow another.
    fn first_star(&mut self, value: U) {
        let prev = self.prev;
        let token = Token::new(Kind::Star, value, Some(w(STAR)));
        let kind = self.tok(prev).kind;
        if self.index == self.start || kind == Kind::Slash || kind == Kind::Dot {
            let guard = if kind == Kind::Dot { NO_DOT_SLASH } else { NO_DOTS_SLASH };
            ext(&mut self.output, guard);
            let p = self.tok_mut(prev);
            ext(p.output.get_or_insert_with(|| w("undefined")), guard);
            if self.peek(1) != Some(STAR_CH) {
                ext(&mut self.output, ONE_CHAR);
                let p = self.tok_mut(prev);
                ext(p.output.get_or_insert_with(|| w("undefined")), ONE_CHAR);
            }
        }
        self.push(token);
    }

    /// A star after a star: a globstar when the pair begins a segment, else nothing more.
    fn second_star(&mut self, value: U) {
        if !self.pair_begins_segment() {
            self.push(Token::new(Kind::Star, value, Some(U::new())));
            return;
        }
        self.globstar(value);
    }

    /// Whether the token before the first star starts a segment: the pattern, a `/`, a
    /// group, a brace's alternative or an extglob.
    fn pair_begins_segment(&self) -> bool {
        let prior = self.tok(self.prev).prev;
        let prior_kind = self.tok(prior).kind;
        let is_start = prior_kind == Kind::Slash || prior_kind == Kind::Bos;
        let is_brace = self.brace_count > 0 && (prior_kind == Kind::Comma || prior_kind == Kind::Brace);
        let is_extglob = !self.extglobs.is_empty() && prior_kind == Kind::Paren;
        is_start || prior_kind == Kind::Paren || is_brace || is_extglob
    }

    fn globstar(&mut self, value: U) {
        let prior = self.tok(self.prev).prev;
        let rest = self.skip_repeated_globstars();
        let prior_kind = self.tok(prior).kind;
        let first_segment = prior_kind == Kind::Bos;
        let inner_segment = prior_kind == Kind::Slash && self.tok(self.tok(prior).prev).kind != Kind::Bos;
        let slash_follows = rest.first() == Some(&SLASH);
        if first_segment && self.eos() {
            self.whole_pattern_globstar(value);
        } else if inner_segment && self.globstar_ends(prior, rest) {
            self.trailing_globstar(prior, value);
        } else if inner_segment && slash_follows {
            self.inner_globstar(prior, value, rest.len() > 1);
        } else if first_segment && slash_follows {
            self.leading_globstar(value);
            self.take_slash();
        } else {
            self.plain_globstar(value);
        }
    }

    /// Strips the `/**` that repeat this one (`a/**/**/b`), returning what follows.
    fn skip_repeated_globstars(&mut self) -> &'a [u16] {
        let mut rest = self.remaining();
        while rest.starts_with(&[SLASH, STAR_CH, STAR_CH]) {
            let after = self.at(self.index + 4);
            if after.is_some_and(|a| a != SLASH) {
                break;
            }
            rest = slice(rest, 3, rest.len());
            self.index += 3;
        }
        rest
    }

    /// The globstar ends the pattern, or the extglobs it closes, and does not follow a star.
    fn globstar_ends(&self, prior: usize, rest: &[u16]) -> bool {
        let before_kind = (prior != BOS).then(|| self.tok(self.tok(prior).prev).kind);
        let after_star = matches!(before_kind, Some(Kind::Star | Kind::Globstar));
        let parens = usize::try_from(self.parens).unwrap_or(0);
        let is_end = self.eos()
            || (self.parens > 0
                && rest.len() == parens
                && rest.iter().all(|&c| c == RPAREN)
                && !self.extglobs.iter().any(|e| e.kind == Kind::Negate));
        !after_star && is_end
    }

    /// `**` alone.
    fn whole_pattern_globstar(&mut self, value: U) {
        let p = self.tok_mut(self.prev);
        p.kind = Kind::Globstar;
        p.value.extend_from_slice(&value);
        p.output = Some(w(GLOBSTAR));
        self.output = w(GLOBSTAR);
    }

    /// `a/**`.
    fn trailing_globstar(&mut self, prior: usize, value: U) {
        let output = w(&format!("{GLOBSTAR}|$)"));
        self.optional_segment_globstar(prior, value, output);
    }

    /// `a/**/b`; `more` when something follows the `/` after it.
    fn inner_globstar(&mut self, prior: usize, value: U, more: bool) {
        let end = if more { "|$" } else { "" };
        let output = w(&format!("{GLOBSTAR}{SLASH_LITERAL}|{SLASH_LITERAL}{end})"));
        self.optional_segment_globstar(prior, value, output);
        self.take_slash();
    }

    /// `a/**` or `a/**/b`: the `/` before it and the globstar become one optional group.
    fn optional_segment_globstar(&mut self, prior: usize, value: U, prev_output: U) {
        let prev = self.prev;
        let both = self.tok(prior).emitted().len() + self.tok(prev).emitted().len();
        drop_last(&mut self.output, both);
        let prior_output = cat(&[&w("(?:"), self.tok(prior).emitted()]);
        self.tok_mut(prior).output = Some(prior_output.clone());
        let p = self.tok_mut(prev);
        p.kind = Kind::Globstar;
        p.output = Some(prev_output.clone());
        p.value.extend_from_slice(&value);
        self.output.extend(prior_output);
        self.output.extend(prev_output);
    }

    /// `**/b`.
    fn leading_globstar(&mut self, value: U) {
        let output = w(&format!("(?:^|{SLASH_LITERAL}|{GLOBSTAR}{SLASH_LITERAL})"));
        let p = self.tok_mut(self.prev);
        p.kind = Kind::Globstar;
        p.value.extend_from_slice(&value);
        p.output = Some(output.clone());
        self.output = output;
    }

    /// The `/` after a globstar, which the globstar's output already matches.
    fn take_slash(&mut self) {
        self.advance();
        self.push(Token::new(Kind::Slash, w("/"), Some(U::new())));
    }

    fn plain_globstar(&mut self, value: U) {
        let prev = self.prev;
        let n = self.tok(prev).emitted().len();
        drop_last(&mut self.output, n);
        let p = self.tok_mut(prev);
        p.kind = Kind::Globstar;
        p.output = Some(w(GLOBSTAR));
        p.value.extend_from_slice(&value);
        ext(&mut self.output, GLOBSTAR);
    }
}
