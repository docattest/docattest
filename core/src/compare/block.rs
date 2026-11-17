//! Where a fenced block on a page first parts from the text a fact observed.

use crate::text::{is_js_space, lines};

/// The first place a page's block and an observed text differ, whitespace ignored: they are
/// compared token by token, a token being a run of letters, digits, `_` and `$` or any other
/// single character, so whitespace matters only where it parts two runs (`let x`, `letx`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// The line of the block (1-based) where they part; one past its last line when the
    /// block ends first.
    pub line: usize,
    /// That line of the block, whitespace collapsed; None when the block ended.
    pub expected: Option<String>,
    /// What the observed text has from the start of that line to the end of its own line
    /// where they part; None when it ended.
    pub actual: Option<String>,
}

/// A token, the line of its text (1-based) it is on, and whether whitespace comes before it.
struct Token<'a> {
    text: &'a str,
    line: usize,
    spaced: bool,
}

/// Where `page` and `observed` first differ token by token, so that wrapping or spacing a
/// block's lines differently is never a difference; None when they have the same tokens.
pub fn first_difference(page: &str, observed: &str) -> Option<Difference> {
    let (page, observed) = (tokens_by_line(page), tokens_by_line(observed));
    let at = (0..page.len().max(observed.len()))
        .find(|&k| page.get(k).map(|t| t.text) != observed.get(k).map(|t| t.text))?;
    let line = page.get(at).map_or_else(|| page.last().map_or(1, |t| t.line + 1), |t| t.line);
    // Every token before `at` is the same on both sides, so the line's first token starts
    // the same stretch of the observed text.
    let from = page.iter().position(|t| t.line == line).unwrap_or(at);
    let expected = page.get(at).map(|_| joined(page.iter().filter(|t| t.line == line)));
    let to = match observed.get(at) {
        Some(token) => observed.iter().rposition(|t| t.line == token.line).map_or(at, |k| k + 1),
        None => observed.len(),
    };
    let actual = (from < to).then(|| joined(observed[from..to].iter()));
    Some(Difference { line, expected, actual })
}

fn tokens_by_line(text: &str) -> Vec<Token<'_>> {
    let mut tokens: Vec<Token> = Vec::new();
    for (k, line) in lines(text).into_iter().enumerate() {
        let mut spaced = true;
        let mut rest = line;
        while let Some(first) = rest.chars().next() {
            let length = if is_run(first) {
                rest.find(|c| !is_run(c)).unwrap_or(rest.len())
            } else {
                first.len_utf8()
            };
            if !is_js_space(first) {
                tokens.push(Token { text: &rest[..length], line: k + 1, spaced });
            }
            spaced = is_js_space(first);
            rest = &rest[length..];
        }
    }
    tokens
}

/// A character of a token that runs on: letters, digits, `_` and `$`.
fn is_run(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Tokens as text, with one space where whitespace parted them.
fn joined<'a>(tokens: impl Iterator<Item = &'a Token<'a>>) -> String {
    let mut out = String::new();
    for token in tokens {
        if token.spaced && !out.is_empty() {
            out.push(' ');
        }
        out.push_str(token.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn difference(line: usize, expected: Option<&str>, actual: Option<&str>) -> Option<Difference> {
        Some(Difference { line, expected: expected.map(str::to_string), actual: actual.map(str::to_string) })
    }

    #[test]
    fn the_same_words_wrapped_differently_do_not_differ() {
        assert_eq!(first_difference("a b\n  c", "a\nb c"), None);
    }

    #[test]
    fn whitespace_matters_only_between_two_runs() {
        assert_eq!(first_difference("[206,176,\n 1]", "[206, 176, 1]"), None);
        assert_eq!(first_difference("f(a)+g( b )", "f (a) + g(b)"), None);
        assert_eq!(
            first_difference("let x = 1", "letx = 1"),
            difference(1, Some("let x = 1"), Some("letx = 1"))
        );
        assert_eq!(first_difference("[1,2]", "[1, 3]"), difference(1, Some("[1,2]"), Some("[1, 3]")));
    }

    #[test]
    fn a_differing_line_is_shown_from_its_start() {
        let page = "type X = {\n  y: number;\n}";
        assert_eq!(
            first_difference(page, "type X = {\n  y: string;\n}"),
            difference(2, Some("y: number;"), Some("y: string;"))
        );
        assert_eq!(
            first_difference(page, "type X = { y: string; }"),
            difference(2, Some("y: number;"), Some("y: string; }"))
        );
    }

    #[test]
    fn either_side_may_end_first() {
        assert_eq!(first_difference("a\nb", "a\nb\nc d"), difference(3, None, Some("c d")));
        assert_eq!(first_difference("a\nb c", "a\nb"), difference(2, Some("b c"), Some("b")));
        assert_eq!(first_difference("a\nb", "a"), difference(2, Some("b"), None));
    }
}
