//! Questions about a piece of Rust text that need only its tokens.

use crate::language::blocks::visible;
use crate::protocol::ExpressionShape;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::str::FromStr;

/// Stands for a name in an expression's shape; no Rust token is spelled so.
const PLACEHOLDER: &str = "\u{b7}";

/// The macros that end a program when what they check does not hold.
pub const ASSERTING_MACROS: [&str; 9] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "assert_matches",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "panic",
    "unreachable",
];

/// The methods that end a program when a value is not what they expect.
pub const ASSERTING_METHODS: [&str; 4] = ["unwrap", "expect", "unwrap_err", "expect_err"];

/// One token of a piece of Rust text.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Ident(String),
    /// Anything else, as written: a literal, a punctuation mark, a delimiter.
    Other(String),
}

impl Token {
    /// The token as written.
    pub fn text(&self) -> &str {
        match self {
            Token::Ident(text) | Token::Other(text) => text,
        }
    }
}

/// The tokens of a piece of Rust text, with what groups hold laid out flat between their
/// delimiters; `None` when it is not Rust's tokens.
pub fn tokens(text: &str) -> Option<Vec<Token>> {
    fn flatten(stream: TokenStream, out: &mut Vec<Token>) {
        for tree in stream {
            match tree {
                TokenTree::Group(group) => {
                    let (open, close) = match group.delimiter() {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Brace => ("{", "}"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::None => ("", ""),
                    };
                    out.push(Token::Other(open.to_string()));
                    flatten(group.stream(), out);
                    out.push(Token::Other(close.to_string()));
                }
                TokenTree::Ident(ident) => out.push(Token::Ident(ident.to_string())),
                other => out.push(Token::Other(other.to_string())),
            }
        }
    }
    let mut out = Vec::new();
    flatten(TokenStream::from_str(text).ok()?, &mut out);
    Some(out)
}

/// Whether a block can fail by its own doing: it calls a macro or a method that ends the
/// program when something is not as expected. A block that cannot be read is not called
/// weak.
pub fn asserts(code: &str) -> bool {
    let Some(tokens) = tokens(&visible(code)) else { return true };
    let is = |k: Option<usize>, mark: &str| {
        k.and_then(|k| tokens.get(k)).is_some_and(|token| token.text() == mark)
    };
    tokens.iter().enumerate().any(|(k, token)| {
        let Token::Ident(name) = token else { return false };
        (ASSERTING_MACROS.contains(&name.as_str()) && is(Some(k + 1), "!"))
            || (ASSERTING_METHODS.contains(&name.as_str())
                && is(k.checked_sub(1), ".")
                && is(Some(k + 1), "("))
    })
}

/// The paths a piece of code writes: each run of identifiers joined by `::`.
pub fn paths(code: &str) -> Vec<Vec<String>> {
    let tokens = tokens(code).unwrap_or_default();
    let colon = |k: usize| tokens.get(k).is_some_and(|token| token.text() == ":");
    let mut paths: Vec<Vec<String>> = Vec::new();
    let mut k = 0;
    while k < tokens.len() {
        let Token::Ident(first) = &tokens[k] else {
            k += 1;
            continue;
        };
        let mut path = vec![first.clone()];
        k += 1;
        while colon(k) && colon(k + 1) {
            let Some(Token::Ident(next)) = tokens.get(k + 2) else { break };
            path.push(next.clone());
            k += 3;
        }
        paths.push(path);
    }
    paths
}

/// The identifiers of a piece of text, in order: every run of letters, digits and `_`
/// that begins with a letter or `_`. It needs no valid Rust, so it reads a code span as
/// it reads a source file.
pub fn identifiers(text: &str) -> Vec<&str> {
    let is_part = |c: char| c.is_alphanumeric() || c == '_';
    text.split(|c: char| !is_part(c))
        .filter(|word| word.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_'))
        .collect()
}

/// For each code span, the first of `symbols` it names as an identifier.
pub fn mentions(spans: &[String], symbols: &[String]) -> Vec<Option<String>> {
    spans
        .iter()
        .map(|span| {
            identifiers(span).into_iter().find(|name| symbols.iter().any(|s| s == name)).map(str::to_string)
        })
        .collect()
}

/// A fact expression with every identifier replaced by a placeholder, and the
/// identifiers in order. An expression that is not Rust's tokens is its own shape.
pub fn shape(expression: &str) -> ExpressionShape {
    let Some(tokens) = tokens(expression) else {
        return ExpressionShape { shape: expression.to_string(), names: Vec::new() };
    };
    let mut names = Vec::new();
    let parts: Vec<&str> = tokens
        .iter()
        .map(|token| match token {
            Token::Ident(name) => {
                names.push(name.clone());
                PLACEHOLDER
            }
            other => other.text(),
        })
        .collect();
    ExpressionShape { shape: parts.join(" "), names }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_asserts_when_something_in_it_can_end_the_program() {
        for code in [
            "assert_eq!(a, 1);",
            "if a { panic!(\"no\") }",
            "let v = r.unwrap();",
            "x.expect(\"it\");",
            "this is { not rust",
        ] {
            assert!(asserts(code), "{code}");
        }
        for code in ["println!(\"{}\", 1);", "let unwrap = 1; f(unwrap);", "# assert!(true);\n"] {
            assert_eq!(asserts(code), code.starts_with('#'), "{code}");
        }
    }

    #[test]
    fn a_span_names_the_first_symbol_it_holds_as_a_word() {
        let spans = ["shop::add(1, 2)", "added", "Money::eur(5).cents", "`total`"].map(String::from);
        let symbols = ["add", "Money", "total"].map(String::from);
        let found = mentions(&spans, &symbols);
        let named: Vec<Option<&str>> = found.iter().map(|name| name.as_deref()).collect();
        assert_eq!(named, [Some("add"), None, Some("Money"), Some("total")]);
    }

    #[test]
    fn a_shape_is_an_expression_without_its_names() {
        let (one, renamed, other) =
            (shape("LIMITS[0].1"), shape("BOUNDS [ 0 ] . 1"), shape("LIMITS[0].1 + 1"));
        assert_eq!(one.shape, renamed.shape);
        assert_ne!(one.shape, other.shape);
        assert_eq!((one.names, renamed.names), (vec!["LIMITS".to_string()], vec!["BOUNDS".to_string()]));
        assert_eq!(shape("a ( b").shape, "a ( b");
    }
}
