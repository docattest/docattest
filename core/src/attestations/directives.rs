//! Directive syntax: `<!-- fact[ modifiers]: <expression> -->`, `<!-- cases: <runner> -->`,
//! `<!-- test: <file> > "<title>" -->` and `<!-- tests: <name> = <file> -->`. Other comments
//! are ordinary comments and are ignored.

use super::alias::{Alias, parse_alias};
use super::citation::{Citation, parse_citation};
use super::field::{FieldPath, parse_field_path};
use super::quoted::{first_outside_quotes, unquoted};
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
    pub field: Option<FieldPath>,
}

/// A parsed directive, or a docattest directive that is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    Fact(FactDirective),
    Cases { runner: String },
    Test(Citation),
    Alias(Alias),
    Invalid { as_kind: Keyword, message: String },
}

/// Parse a comment body; None when the comment is not a docattest directive.
pub fn parse_directive(body: &str, line: usize) -> Option<Directive> {
    let text = trim(body);
    let word = leading_keyword(text)?;
    match Keyword::named(word) {
        Some(Keyword::Cases) => Some(parse_cases(text, line)),
        Some(Keyword::Fact) => Some(parse_fact(&text[word.len()..], line)),
        Some(Keyword::Test) => parse_citation(text, line),
        Some(Keyword::Tests) => parse_alias(text, line),
        None => near_miss(word, line),
    }
}

/// A word one letter from a keyword, written where a directive would be: an error that says
/// which keyword was meant. None for any other word.
fn near_miss(word: &str, line: usize) -> Option<Directive> {
    let meant = match word {
        "facts" => Keyword::Fact,
        "case" => Keyword::Cases,
        _ => return None,
    };
    let message = format!("unknown directive \"{word}\" at L{line}; did you mean \"{}\"?", meant.as_str());
    Some(Directive::Invalid { as_kind: Keyword::Fact, message })
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
    let (column, field, remainder) = match take_selectors(rest) {
        Ok(taken) => taken,
        Err(why) => return invalid(format!("fact at L{line}: {why}")),
    };
    let bare_column = column.as_ref().filter(|c| !c.quoted).map(|c| c.name.as_str());
    let Some(colon) = remainder.find(':') else {
        return invalid(format!("fact at L{line} has no expression; expected <!-- fact: <expression> -->"));
    };
    let (surface, ordered) = match read_modifiers(&words(trim(&remainder[..colon])), bare_column) {
        Ok(read) => read,
        Err(why) => return invalid(format!("fact at L{line}: {why}")),
    };
    let expression = trim(&remainder[colon + 1..]);
    if expression.is_empty() {
        return invalid(format!("fact at L{line} has no expression"));
    }
    let column = column.map(|c| c.name);
    Directive::Fact(FactDirective { expression: expression.to_string(), surface, ordered, column, field })
}

/// The surface and `ordered` of a fact, from the words before its colon that are not
/// `column=` or `field=`. A fact reads one surface and selects with one `column=` or
/// `field=`, so a second of either is refused rather than one silently winning.
fn read_modifiers(modifiers: &[&str], bare_column: Option<&str>) -> Result<(String, bool), String> {
    let mut surface: Option<&str> = None;
    let mut ordered = false;
    for word in modifiers {
        if SELECTORS.iter().any(|key| word.starts_with(key)) {
            return Err(format!("a fact selects with one column= or field=; \"{word}\" is a second"));
        }
        if !is_modifier(word) {
            let unknown = format!("unknown modifier \"{word}\"");
            return Err(match bare_column {
                Some(name) => format!("{unknown}; {}", quoting_hint(name, modifiers)),
                None => unknown,
            });
        }
        match (word.strip_prefix('@'), surface) {
            (Some(name), Some(first)) => {
                return Err(format!("a fact reads one surface; it names @{first} and @{name}"));
            }
            (Some(name), None) => surface = Some(name),
            (None, _) => ordered = true,
        }
    }
    Ok((surface.unwrap_or(DEFAULT_SURFACE).to_string(), ordered))
}

/// The modifiers that pick what a fact binds.
const SELECTORS: [&str; 2] = ["column=", "field="];

/// `column=` and `field=`, which pick what a fact binds; at most one of them.
fn take_selectors(rest: &str) -> Result<(Option<ColumnName>, Option<FieldPath>, String), String> {
    let (field, remainder) = take_value(rest, "field=");
    let (column, remainder) = take_value(&remainder, "column=");
    let field = field.as_deref().map(parse_field_path).transpose()?;
    if column.is_some() && field.is_some() {
        return Err("column= and field= cannot both be given".into());
    }
    let column = column.map(|written| match unquoted(&written) {
        Some(name) => ColumnName { name: name.to_string(), quoted: true },
        None => ColumnName { name: written, quoted: false },
    });
    if column.as_ref().is_some_and(|c| c.name.is_empty()) {
        return Err("column= names no column".into());
    }
    Ok((column, field, remainder))
}

/// `@<surface>` or `ordered`.
fn is_modifier(word: &str) -> bool {
    word == "ordered" || word.strip_prefix('@').is_some_and(|name| !name.is_empty())
}

/// How to write a column name with spaces, from a bare `column=` and the words after it that
/// are no modifier: they read as the rest of the name.
fn quoting_hint(bare: &str, modifiers: &[&str]) -> String {
    let rest = modifiers.iter().copied().filter(|word| !is_modifier(word));
    let name = std::iter::once(bare).chain(rest).collect::<Vec<_>>().join(" ");
    format!("a column name with spaces is quoted: column=\"{name}\"")
}

/// The value of `column=` and whether it was written in quotes.
struct ColumnName {
    name: String,
    quoted: bool,
}

// `column=` and `field=` count only before the expression's colon. A value runs to the first
// space or colon outside quotes.
fn take_value(rest: &str, key: &str) -> (Option<String>, String) {
    let colon = rest.find(':');
    let mut from = 0;
    while let Some(found) = rest.get(from..).and_then(|r| r.find(key)) {
        let at = from + found;
        from = at + 1;
        if rest[..at].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let value_start = at + key.len();
        let tail = &rest[value_start..];
        let length = first_outside_quotes(tail, |c| is_js_space(c) || c == ':').unwrap_or(tail.len());
        if length == 0 {
            continue;
        }
        // The first one decides: after the colon, or with no colon at all, there is none.
        if colon.is_none_or(|c| at > c) {
            return (None, rest.to_string());
        }
        let remainder = format!("{}{}", &rest[..at], &rest[value_start + length..]);
        return (Some(tail[..length].to_string()), remainder);
    }
    (None, rest.to_string())
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
            field: None,
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
        assert_eq!(
            message("fact column=program address ordered: N", 4),
            "fact at L4: unknown modifier \"address\"; a column name with spaces is quoted: \
             column=\"program address\""
        );
        assert_eq!(message("fact column='a b' c: N", 4), "fact at L4: unknown modifier \"c\"");
        assert_eq!(
            parse_directive("fact column='program address': N", 1),
            Some(fact("N", "default", false, Some("program address")))
        );
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

    #[test]
    fn a_field_path_may_quote_a_key() {
        let field = |body: &str| match parse_directive(body, 1) {
            Some(Directive::Fact(f)) => f.field.map(|path| path.written),
            other => panic!("{other:?}"),
        };
        assert_eq!(field("fact field=ports.0.port: PORT"), Some("ports.0.port".into()));
        assert_eq!(
            field("fact field=servers.\"eu west\".port ordered: P"),
            Some("servers.\"eu west\".port".into())
        );
        assert_eq!(field("fact @api field='a:b': P"), Some("'a:b'".into()));
        assert_eq!(field("fact: f(\"field=x\")"), None);
        assert_eq!(message("fact field=a..b: P", 4), "fact at L4: field=a..b has an empty key");
        assert!(message("fact column=k field=a: P", 4).contains("column= and field= cannot both be given"));
    }

    #[test]
    fn a_second_surface_or_selector_and_an_empty_column_are_refused() {
        assert_eq!(
            message("fact @api @shipping: P", 4),
            "fact at L4: a fact reads one surface; it names @api and @shipping"
        );
        assert_eq!(
            message("fact column=zone column=code: P", 4),
            "fact at L4: a fact selects with one column= or field=; \"column=code\" is a second"
        );
        assert_eq!(
            message("fact field=a field=b: P", 4),
            "fact at L4: a fact selects with one column= or field=; \"field=b\" is a second"
        );
        assert_eq!(message("fact column=\"\": P", 4), "fact at L4: column= names no column");
    }
}
