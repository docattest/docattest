//! parse.js: the parser's state and its cursor; the main loop (`run`), the handlers it
//! calls for each kind of character (`groups`, `star`, `handlers`) and extglobs
//! (`extglobs`) are beside it.

mod extglobs;
mod groups;
mod handlers;
mod run;
mod star;

use super::constants::{STAR, apply_replacements, length_error, remove_prefix};
use super::make_re::fast_regex;
use super::units::{BANG, LPAREN, QMARK_CH, STAR_CH, drop_last, ext, is, slice, w};
use super::{MAX_LENGTH, U};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Bos,
    Text,
    Paren,
    Bracket,
    Brace,
    Comma,
    Slash,
    Dot,
    Dots,
    Qmark,
    Negate,
    Plus,
    Star,
    Globstar,
    At,
    MaybeSlash,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Counter {
    Parens,
    Brackets,
    Braces,
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    value: U,
    /// `None` is `undefined`: the value stands in for the output.
    output: Option<U>,
    prev: usize,
    extglob: bool,
    star: bool,
    posix: bool,
    dots: bool,
    comma: bool,
    output_index: usize,
    tokens_index: usize,
}

impl Token {
    fn new(kind: Kind, value: U, output: Option<U>) -> Token {
        Token {
            kind,
            value,
            output,
            prev: 0,
            extglob: false,
            star: false,
            posix: false,
            dots: false,
            comma: false,
            output_index: 0,
            tokens_index: 0,
        }
    }

    fn text(value: U) -> Token {
        Token::new(Kind::Text, value, None)
    }

    /// `token.output != null ? token.output : token.value`.
    fn emitted(&self) -> &[u16] {
        self.output.as_deref().unwrap_or(&self.value)
    }

    /// `token.output || token.value`.
    fn output_or_value(&self) -> &[u16] {
        match &self.output {
            Some(o) if !o.is_empty() => o,
            _ => &self.value,
        }
    }
}

struct Extglob {
    kind: Kind,
    close: U,
    inner: U,
    parens: i64,
    output: U,
    start_index: i64,
    tokens_index: usize,
}

pub(super) struct Parsed {
    pub(super) output: U,
    pub(super) negated: bool,
}

/// `parse(input, { dot: true, fastpaths })`.
pub(super) fn parse(input: &[u16], fastpaths: bool) -> Result<Parsed, String> {
    let replaced = apply_replacements(input);
    if replaced.len() > MAX_LENGTH {
        return Err(length_error(replaced.len()));
    }
    let input = remove_prefix(&replaced);
    // `!/(^[*!]|[/()[\]{}"])/.test(input)`
    let simple = !matches!(input.first(), Some(&c) if c == STAR_CH || c == BANG)
        && !input.iter().any(|&c| "/()[]{}\"".encode_utf16().any(|s| s == c));
    if fastpaths && simple {
        return Ok(Parsed { output: fast_regex(input), negated: false });
    }
    let mut parser = Parser::new(input);
    parser.run()?;
    Ok(Parsed { output: parser.output, negated: parser.negated })
}

struct Parser<'a> {
    input: &'a [u16],
    arena: Vec<Token>,
    tokens: Vec<usize>,
    prev: usize,
    extglobs: Vec<Extglob>,
    braces: Vec<usize>,
    stack: Vec<Counter>,
    index: i64,
    start: i64,
    output: U,
    backtrack: bool,
    negated: bool,
    brackets: i64,
    brace_count: i64,
    parens: i64,
    quotes: i64,
}

const BOS: usize = 0;

impl<'a> Parser<'a> {
    fn new(input: &'a [u16]) -> Parser<'a> {
        Parser {
            input,
            arena: vec![Token::new(Kind::Bos, U::new(), Some(U::new()))],
            tokens: vec![BOS],
            prev: BOS,
            extglobs: Vec::new(),
            braces: Vec::new(),
            stack: Vec::new(),
            index: -1,
            start: 0,
            output: U::new(),
            backtrack: false,
            negated: false,
            brackets: 0,
            brace_count: 0,
            parens: 0,
            quotes: 0,
        }
    }

    fn len(&self) -> i64 {
        i64::try_from(self.input.len()).unwrap_or(i64::MAX)
    }

    fn at(&self, i: i64) -> Option<u16> {
        usize::try_from(i).ok().and_then(|i| self.input.get(i).copied())
    }

    fn peek(&self, n: i64) -> Option<u16> {
        self.at(self.index + n)
    }

    fn advance(&mut self) -> U {
        self.index += 1;
        self.at(self.index).map(|c| vec![c]).unwrap_or_default()
    }

    fn remaining(&self) -> &'a [u16] {
        let from = usize::try_from(self.index + 1).unwrap_or(0);
        slice(self.input, from, self.input.len())
    }

    fn eos(&self) -> bool {
        self.index == self.len() - 1
    }

    fn tok(&self, id: usize) -> &Token {
        &self.arena[id]
    }

    fn tok_mut(&mut self, id: usize) -> &mut Token {
        &mut self.arena[id]
    }

    fn prev_value_is(&self, s: &str) -> bool {
        is(&self.tok(self.prev).value, s)
    }

    fn increment(&mut self, counter: Counter) {
        match counter {
            Counter::Parens => self.parens += 1,
            Counter::Brackets => self.brackets += 1,
            Counter::Braces => self.brace_count += 1,
        }
        self.stack.push(counter);
    }

    fn decrement(&mut self, counter: Counter) {
        match counter {
            Counter::Parens => self.parens -= 1,
            Counter::Brackets => self.brackets -= 1,
            Counter::Braces => self.brace_count -= 1,
        }
        self.stack.pop();
    }

    fn append(&mut self, piece: &[u16]) {
        self.output.extend_from_slice(piece);
    }

    /// `push(tok)`; returns the token's id unless it merged into the previous text.
    fn push(&mut self, mut tok: Token) -> Option<usize> {
        let prev = self.prev;
        if self.tok(prev).kind == Kind::Globstar && !self.globstar_stands_before(&tok) {
            self.globstar_to_star(prev);
        }
        if tok.kind != Kind::Paren
            && let Some(last) = self.extglobs.last_mut()
        {
            last.inner.extend_from_slice(&tok.value);
        }
        if !tok.value.is_empty() || tok.output.as_ref().is_some_and(|o| !o.is_empty()) {
            let piece = tok.emitted().to_vec();
            self.append(&piece);
        }
        if self.tok(prev).kind == Kind::Text && tok.kind == Kind::Text {
            self.merge_text(prev, &tok.value);
            return None;
        }
        tok.prev = prev;
        let id = self.arena.len();
        self.arena.push(tok);
        self.tokens.push(id);
        self.prev = id;
        Some(id)
    }

    /// A globstar stays one before a `/`, a group, a brace's alternative or an extglob.
    fn globstar_stands_before(&self, tok: &Token) -> bool {
        let is_brace = self.brace_count > 0 && (tok.kind == Kind::Comma || tok.kind == Kind::Brace);
        let is_extglob = tok.extglob || (!self.extglobs.is_empty() && tok.kind == Kind::Paren);
        tok.kind == Kind::Slash || tok.kind == Kind::Paren || is_brace || is_extglob
    }

    /// Anything else after a globstar makes it a plain star.
    fn globstar_to_star(&mut self, prev: usize) {
        let n = self.tok(prev).output.as_ref().map_or(0, Vec::len);
        drop_last(&mut self.output, n);
        let p = self.tok_mut(prev);
        p.kind = Kind::Star;
        p.value = w("*");
        p.output = Some(w(STAR));
        ext(&mut self.output, STAR);
    }

    /// Text after text joins it: `prev.output = (prev.output || prev.value) + tok.value`.
    fn merge_text(&mut self, prev: usize, value: &[u16]) {
        let p = self.tok_mut(prev);
        let mut merged = match p.output.take() {
            Some(o) if !o.is_empty() => o,
            _ => p.value.clone(),
        };
        merged.extend_from_slice(value);
        p.output = Some(merged);
        p.value.extend_from_slice(value);
    }

    fn negate(&mut self) {
        let mut count = 1;
        while self.peek(1) == Some(BANG) && (self.peek(2) != Some(LPAREN) || self.peek(3) == Some(QMARK_CH)) {
            self.advance();
            self.start += 1;
            count += 1;
        }
        if count % 2 == 0 {
            return;
        }
        self.negated = true;
        self.start += 1;
    }
}
