//! JavaScript RegExp (flagless, Annex B) to fancy_regex: compiling picomatch's source,
//! and the translator's state. Its grammar is split by what it reads: terms and
//! quantifiers, groups, escapes, character classes.

mod classes;
mod escapes;
mod groups;
mod terms;

use fancy_regex::{Regex, RegexBuilder};

use super::BACKTRACK_LIMIT;
use super::units::{BACKSLASH, BANG, EQUALS, LBRACKET, LPAREN, LT, QMARK_CH, RBRACKET};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RegexError {
    /// V8 throws a SyntaxError.
    Syntax,
    /// Valid JavaScript that this translation cannot express.
    Unsupported,
}

type Rx<T> = Result<T, RegexError>;

/// Compiles picomatch's regex source, or `None` where JavaScript's `/$^/` fallback applies.
pub(super) fn compile(source: &[u16]) -> Option<Regex> {
    build(&translate(source).ok()?).ok()
}

fn build(translated: &str) -> Result<Regex, String> {
    RegexBuilder::new(translated).backtrack_limit(BACKTRACK_LIMIT).build().map_err(|e| e.to_string())
}

/// Where this port may part from JavaScript for a regex source, and why.
#[cfg(test)]
pub(super) fn infidelity(source: &[u16]) -> Option<String> {
    match translate_full(source) {
        Err(RegexError::Syntax) => None,
        Err(RegexError::Unsupported) => Some("unsupported".into()),
        Ok(t) => match build(&t.source) {
            Err(e) if e.contains("Look-behind") => Some("lookbehind".into()),
            Err(e) => Some(format!("fancy-regex: {e}")),
            Ok(_) => t.capture_reset.then(|| "capture-reset".into()),
        },
    }
}

/// Nesting deeper than this is reported unsupported (fancy_regex stops at 64 anyway).
const MAX_DEPTH: usize = 200;
/// V8's `RegExpTree::kInfinity`.
const K_INFINITY: u64 = 0x7FFF_FFFF;
const K_MAX_CAPTURES: u64 = 1 << 16;

struct Frag {
    s: String,
    /// The atom can only ever match the empty string.
    zero_width: bool,
    /// For a non-capturing group of one alternative: its content, usable without parens.
    bare: Option<String>,
}

impl Frag {
    fn atom(s: String) -> Frag {
        Frag { s, zero_width: false, bare: None }
    }
}

enum Term {
    Assertion(String),
    Atom(Frag),
}

enum ClassAtom {
    Unit(u16),
    Set(Vec<(u16, u16)>),
}

struct Disjunction {
    s: String,
    zero_width: bool,
    parts: Vec<String>,
}

struct Translator<'a> {
    s: &'a [u16],
    i: usize,
    total_captures: usize,
    has_named: bool,
    captures_started: usize,
    open: Vec<(usize, Option<Vec<u32>>)>,
    names: Vec<(Vec<u32>, usize)>,
    known: Option<Vec<(Vec<u32>, usize)>>,
    references: Vec<Vec<u32>>,
    states: Vec<GroupState>,
    depth: usize,
    /// Capture groups (first, last) inside atoms that repeat.
    repeated: Vec<(usize, usize)>,
    /// Groups that backreferences name.
    backreferences: Vec<usize>,
}

/// V8's `RegExpParserState`, as far as its duplicate-name check goes: a named group closing
/// may reuse a name only if every earlier group with it falls in `interval` (`(start, end]`),
/// which `|` extends over the groups of the alternatives before it.
#[derive(Clone, Copy)]
struct GroupState {
    capture_index: usize,
    interval: (usize, usize),
}

/// V8's `ScanForCaptures`: the number of capturing groups and whether any is named.
fn prescan(s: &[u16]) -> (usize, bool) {
    let (mut count, mut named) = (0, false);
    let mut i = 0;
    while let Some(&c) = s.get(i) {
        i += 1;
        if c == BACKSLASH {
            i += 1;
        } else if c == LBRACKET {
            i = skip_class(s, i);
        } else if c == LPAREN {
            let (next, capture) = scan_group(s, i);
            i = next;
            if let Some(is_named) = capture {
                count += 1;
                named |= is_named;
            }
        }
    }
    (count, named)
}

/// Just past a class's `[`: where its `]` ends it.
fn skip_class(s: &[u16], mut i: usize) -> usize {
    while let Some(&d) = s.get(i) {
        i += 1;
        if d == BACKSLASH {
            i += 1;
        } else if d == RBRACKET {
            break;
        }
    }
    i
}

/// Just past a group's `(`: where scanning goes on, and whether the group captures
/// (`Some`) and is named (`Some(true)`).
fn scan_group(s: &[u16], mut i: usize) -> (usize, Option<bool>) {
    if s.get(i) != Some(&QMARK_CH) {
        return (i, Some(false));
    }
    i += 1;
    if s.get(i) != Some(&LT) {
        return (i, None);
    }
    i += 1;
    if matches!(s.get(i), Some(&d) if d == EQUALS || d == BANG) {
        return (i, None);
    }
    (i, Some(true))
}

/// Rewrites a JavaScript regex source for fancy_regex, or reports why it cannot.
pub(super) fn translate(source: &[u16]) -> Rx<String> {
    translate_full(source).map(|t| t.source)
}

struct Translation {
    source: String,
    /// A backreference names a group inside a repeated atom. JavaScript clears such a
    /// group at the start of every iteration; fancy_regex keeps the last value it took, so
    /// results can differ.
    #[cfg_attr(not(test), allow(dead_code))]
    capture_reset: bool,
}

fn translate_full(source: &[u16]) -> Rx<Translation> {
    let (total, has_named) = prescan(source);
    let mut t = Translator::new(source, total, has_named, None);
    let mut out = t.run()?;
    if has_named {
        let known = t.names.clone();
        t = Translator::new(source, total, has_named, Some(known));
        out = t.run()?;
    }
    let capture_reset =
        t.backreferences.iter().any(|g| t.repeated.iter().any(|(first, last)| first <= g && g <= last));
    Ok(Translation { source: out, capture_reset })
}

fn is_id_start(cp: u32) -> bool {
    match char::from_u32(cp) {
        Some(c) if c.is_ascii() => c.is_ascii_alphabetic() || c == '$' || c == '_',
        Some(c) => c.is_alphabetic(),
        None => false,
    }
}

fn is_id_continue(cp: u32) -> bool {
    match char::from_u32(cp) {
        Some(c) if c.is_ascii() => c.is_ascii_alphanumeric() || c == '$' || c == '_',
        Some(c) => c.is_alphanumeric() || c == '\u{200C}' || c == '\u{200D}',
        None => false,
    }
}

fn hex_value(c: u16) -> Option<u32> {
    char::from_u32(u32::from(c)).and_then(|c| c.to_digit(16))
}

fn is_digit(c: Option<u16>) -> bool {
    c.is_some_and(|c| (u16::from(b'0')..=u16::from(b'9')).contains(&c))
}

fn is_octal(c: Option<u16>) -> bool {
    c.is_some_and(|c| (u16::from(b'0')..=u16::from(b'7')).contains(&c))
}

impl<'a> Translator<'a> {
    fn new(
        s: &'a [u16],
        total_captures: usize,
        has_named: bool,
        known: Option<Vec<(Vec<u32>, usize)>>,
    ) -> Translator<'a> {
        Translator {
            s,
            i: 0,
            total_captures,
            has_named,
            captures_started: 0,
            open: Vec::new(),
            names: Vec::new(),
            known,
            references: Vec::new(),
            states: vec![GroupState { capture_index: 0, interval: (0, 0) }],
            depth: 0,
            repeated: Vec::new(),
            backreferences: Vec::new(),
        }
    }

    fn cur(&self) -> Option<u16> {
        self.s.get(self.i).copied()
    }

    fn at(&self, i: usize) -> Option<u16> {
        self.s.get(i).copied()
    }
}
