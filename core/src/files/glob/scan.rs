//! scan.js (only what `isGlob` needs, with default options).

use super::units::{
    AT, BACKSLASH, BANG, COMMA, DOT, LBRACE, LBRACKET, LPAREN, PLUS_CH, QMARK_CH, RBRACE, RBRACKET, SLASH,
    STAR_CH,
};

pub(super) struct Scanner<'a> {
    s: &'a [u16],
    index: i64,
    code: Option<u16>,
    prev: Option<u16>,
}

impl<'a> Scanner<'a> {
    pub(super) fn new(s: &'a [u16]) -> Scanner<'a> {
        Scanner { s, index: -1, code: None, prev: None }
    }

    fn last(&self) -> i64 {
        i64::try_from(self.s.len()).unwrap_or(i64::MAX) - 1
    }

    fn eos(&self) -> bool {
        self.index >= self.last()
    }

    fn at(&self, i: i64) -> Option<u16> {
        usize::try_from(i).ok().and_then(|i| self.s.get(i).copied())
    }

    fn peek(&self) -> Option<u16> {
        self.at(self.index + 1)
    }

    /// `advance()`; `None` is `NaN`.
    fn advance(&mut self) -> Option<u16> {
        self.prev = self.code;
        self.index += 1;
        self.code = self.at(self.index);
        self.code
    }

    /// `while (eos() !== true && (code = advance()))`: NaN and NUL both end the loop.
    fn advance_truthy(&mut self) -> bool {
        !self.eos() && self.advance().is_some_and(|c| c != 0)
    }

    pub(super) fn is_glob(&mut self) -> bool {
        let mut start: i64 = 0;
        let mut brace_escaped = false;
        while self.index < self.last() {
            let code = self.advance();
            if code == Some(BACKSLASH) {
                if self.advance() == Some(LBRACE) {
                    brace_escaped = true;
                }
                continue;
            }
            if brace_escaped || code == Some(LBRACE) {
                return self.brace_is_glob(brace_escaped);
            }
            if let Some(found) = self.glob_character(code) {
                return found;
            }
            self.track_start(code, &mut start);
        }
        false
    }

    /// Inside braces, or after an escaped `{`: a range `..` or, unless escaped, a `,`
    /// makes a glob.
    fn brace_is_glob(&mut self, escaped: bool) -> bool {
        let mut braces = 1;
        while self.advance_truthy() {
            let code = self.code;
            if code == Some(BACKSLASH) {
                self.advance();
                continue;
            }
            if code == Some(LBRACE) {
                braces += 1;
                continue;
            }
            if !escaped && self.range_or_comma(code) {
                return true;
            }
            if self.code == Some(RBRACE) {
                braces -= 1;
                if braces == 0 {
                    break;
                }
            }
        }
        false
    }

    /// `..` (read past) or `,`.
    fn range_or_comma(&mut self, code: Option<u16>) -> bool {
        (code == Some(DOT) && self.advance() == Some(DOT)) || self.code == Some(COMMA)
    }

    /// What a character outside braces says once it settles the answer: an extglob, a
    /// star, a question mark, a class or a group.
    fn glob_character(&mut self, code: Option<u16>) -> Option<bool> {
        let extglob_char = code.is_some_and(|c| [PLUS_CH, AT, STAR_CH, QMARK_CH, BANG].contains(&c));
        if extglob_char && self.peek() == Some(LPAREN) {
            return Some(true);
        }
        if code == Some(STAR_CH) || code == Some(QMARK_CH) {
            return Some(true);
        }
        if code == Some(LBRACKET) {
            return Some(self.class_closes());
        }
        (code == Some(LPAREN)).then_some(true)
    }

    /// After `[`: whether a `]` closes the class.
    fn class_closes(&mut self) -> bool {
        while self.advance_truthy() {
            if self.code == Some(BACKSLASH) {
                self.advance();
                continue;
            }
            if self.code == Some(RBRACKET) {
                return true;
            }
        }
        false
    }

    /// Moves `start` past a leading `./` and a leading `!`.
    fn track_start(&self, code: Option<u16>, start: &mut i64) {
        if code == Some(SLASH) {
            if self.prev == Some(DOT) && self.index == *start + 1 {
                *start += 2;
            }
        } else if code == Some(BANG) && self.index == *start {
            *start += 1;
        }
    }
}
