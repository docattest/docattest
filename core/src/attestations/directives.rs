//! Directive syntax: `<!-- fact[ modifiers]: <expression> -->`, `<!-- cases: <runner> -->` and
//! `<!-- test: <file> > "<title>" -->`. Other comments are ordinary comments and are ignored.

use super::citation::{Citation, parse_citation};
use crate::config::DEFAULT_SURFACE;
use crate::markdown::model::Keyword;
use crate::text::{is_js_space, is_word, trim, words};

/// A fact directive's parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDirective {
    pub expression: String,
    pub surface: String,
    pub ordered: bool,
    pub column: Option<String>,
}

/// A parsed directive, or a docattest directive that is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    Fact(FactDirective),
    Cases { runner: String },
    Test(Citation),
    Invalid { as_kind: Keyword, message: String },
}

/// Parse a comment body; None when the comment is not a docattest directive.
pub fn parse_directive(body: &str, line: usize) -> Option<Directive> {
    let text = trim(body);
    let keyword = leading_keyword(text)?;
    match keyword {
        "cases" => Some(parse_cases(text, line)),
        "fact" => Some(parse_fact(&text[4..], line)),
        "test" => parse_citation(text, line),
        "facts" | "case" => {
            let meant = if keyword == "facts" { "fact" } else { "cases" };
            let message = format!("unknown directive \"{keyword}\" at L{line}; did you mean \"{meant}\"?");
            Some(Directive::Invalid { as_kind: Keyword::Fact, message })
        }
        _ => None,
    }
}

/// `/^([A-Za-z]+)\b/`: the leading letters, when a word boundary follows them.
fn leading_keyword(text: &str) -> Option<&str> {
    let end = text.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(text.len());
    if end == 0 || text[end..].chars().next().is_some_and(is_word) {
        return None;
    }
    Some(&text[..end])
}

/// `/^cases\s*:\s*([A-Za-z_$][\w$]*)\s*$/`
fn parse_cases(text: &str, line: usize) -> Directive {
    let runner = text
        .strip_prefix("cases")
        .map(|r| r.trim_start_matches(is_js_space))
        .and_then(|r| r.strip_prefix(':'))
        .map(|r| r.trim_start_matches(is_js_space))
        .and_then(|r| {
            let end = r.find(|c: char| !(is_word(c) || c == '$')).unwrap_or(r.len());
            let (name, rest) = r.split_at(end);
            let first_ok =
                name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$');
            (first_ok && rest.chars().all(is_js_space)).then(|| name.to_string())
        });
    match runner {
        Some(runner) => Directive::Cases { runner },
        None => Directive::Invalid {
            as_kind: Keyword::Cases,
            message: format!("cases at L{line}: expected <!-- cases: <runner> -->"),
        },
    }
}

fn parse_fact(rest: &str, line: usize) -> Directive {
    let invalid = |message: String| Directive::Invalid { as_kind: Keyword::Fact, message };
    let (column, remainder) = take_column(rest);
    let Some(colon) = remainder.find(':') else {
        return invalid(format!("fact at L{line} has no expression; expected <!-- fact: <expression> -->"));
    };
    let mut surface = DEFAULT_SURFACE.to_string();
    let mut ordered = false;
    for word in words(trim(&remainder[..colon])) {
        if let Some(name) = word.strip_prefix('@')
            && !name.is_empty()
        {
            surface = name.to_string();
        } else if word == "ordered" {
            ordered = true;
        } else {
            return invalid(format!("fact at L{line}: unknown modifier \"{word}\""));
        }
    }
    let expression = trim(&remainder[colon + 1..]);
    if expression.is_empty() {
        return invalid(format!("fact at L{line} has no expression"));
    }
    let column = column.filter(|c| !c.is_empty());
    Directive::Fact(FactDirective { expression: expression.to_string(), surface, ordered, column })
}

// column= may quote its value, and only counts when it comes before the expression's colon.
// `/\bcolumn=(?:"([^"]*)"|'([^']*)'|([^\s:]+))/`
fn take_column(rest: &str) -> (Option<String>, String) {
    let colon = rest.find(':');
    let mut from = 0;
    while let Some(found) = rest.get(from..).and_then(|r| r.find("column=")) {
        let at = from + found;
        from = at + 1;
        if rest[..at].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let value_start = at + "column=".len();
        let Some((value, end)) = column_value(rest, value_start) else {
            continue;
        };
        // The first `column=` decides: after the colon, or with no colon at all, there is
        // no column.
        if colon.is_none_or(|c| at > c) {
            return (None, rest.to_string());
        }
        let remainder = format!("{}{}", &rest[..at], &rest[end..]);
        return (Some(value), remainder);
    }
    (None, rest.to_string())
}

fn column_value(rest: &str, start: usize) -> Option<(String, usize)> {
    let tail = &rest[start..];
    for quote in ['"', '\''] {
        if let Some(inner) = tail.strip_prefix(quote)
            && let Some(close) = inner.find(quote)
        {
            return Some((inner[..close].to_string(), start + 1 + close + 1));
        }
    }
    let end = tail.find(|c: char| is_js_space(c) || c == ':').unwrap_or(tail.len());
    (end > 0).then(|| (tail[..end].to_string(), start + end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(e: &str, s: &str, o: bool, c: Option<&str>) -> Directive {
        Directive::Fact(FactDirective {
            expression: e.into(),
            surface: s.into(),
            ordered: o,
            column: c.map(Into::into),
        })
    }

    fn message(body: &str, line: usize) -> String {
        match parse_directive(body, line) {
            Some(Directive::Invalid { message, .. }) => message,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn ordinary_comments_are_ignored() {
        assert_eq!(parse_directive(" a note for editors ", 1), None);
        assert_eq!(parse_directive("", 1), None);
    }

    #[test]
    fn facts_with_modifiers() {
        assert_eq!(parse_directive(" fact: N ", 1), Some(fact("N", "default", false, None)));
        assert_eq!(
            parse_directive("fact @server ordered: KEYS", 1),
            Some(fact("KEYS", "server", true, None))
        );
        assert_eq!(
            parse_directive("fact column=\"Field name\": rows()", 1),
            Some(fact("rows()", "default", false, Some("Field name")))
        );
        assert_eq!(
            parse_directive(" fact:\n  errorFields() ", 1),
            Some(fact("errorFields()", "default", false, None))
        );
        assert_eq!(
            parse_directive("fact: f(\"column=x\")", 1),
            Some(fact("f(\"column=x\")", "default", false, None))
        );
    }

    #[test]
    fn malformed_directives() {
        assert!(message("fact N", 4).contains("L4 has no expression"));
        assert_eq!(message("fact sorted: N", 4), "fact at L4: unknown modifier \"sorted\"");
        assert_eq!(message("fact: ", 4), "fact at L4 has no expression");
        assert!(matches!(
            parse_directive("cases: two words", 4),
            Some(Directive::Invalid { as_kind: Keyword::Cases, .. })
        ));
        assert!(message("facts: N", 2).contains("did you mean \"fact\""));
        assert!(message("case: t", 2).contains("did you mean \"cases\""));
        assert_eq!(parse_directive("cases: run", 1), Some(Directive::Cases { runner: "run".into() }));
        assert_eq!(parse_directive("fact2: x", 1), None);
    }
}
