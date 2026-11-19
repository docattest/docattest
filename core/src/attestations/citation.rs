//! Citation syntax: `<!-- test: <file> > "<describe>" > … > "<title>" -->`, where the last
//! segment may be a set of titles, `{"<title>", "<title>"}`, each cited as if written apart.

use super::directives::Directive;
use crate::files::source::{normalise, posix_normalize};
use crate::markdown::model::Keyword;
use crate::text::{is_js_space, prefix, trim};

/// How much of a malformed citation a message shows, in characters.
const SHOWN: usize = 40;

/// A parsed citation: the test file, the titles of the blocks around the tests, outermost
/// first, and the titles of the tests it cites there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// As written: a path relative to the project root, or a page's alias for one.
    pub file: String,
    pub describes: Vec<String>,
    /// One title, or each title of a set, in the order written.
    pub titles: Vec<String>,
}

const FORM: &str = "expected <!-- test: <file> > \"<title>\" -->";

/// Parse a comment body that starts with the word `test`. A colon after it makes it a
/// citation; otherwise it is an ordinary comment, unless it is plainly a citation that lost
/// its colon. None for ordinary comments.
pub fn parse_citation(text: &str, line: usize) -> Option<Directive> {
    let invalid = |why: String| Directive::Invalid {
        as_kind: Keyword::Test,
        message: format!("test at L{line} {why}; {FORM}"),
    };
    // /^test\s*:/
    let after = text.strip_prefix("test")?;
    let Some(rest) = after.trim_start_matches(is_js_space).strip_prefix(':') else {
        return if near_miss(after) { Some(invalid("has no colon after \"test\"".into())) } else { None };
    };
    Some(match citation(rest) {
        Ok(citation) => Directive::Test(citation),
        Err(why) => invalid(why),
    })
}

/// What follows `test:`: the file, then the titles; Err says what is wrong instead.
fn citation(rest: &str) -> Result<Citation, String> {
    let arrow = rest.find('>');
    let file = trim(arrow.map_or(rest, |a| &rest[..a]));
    let clean = cited_file(file)?;
    let no_title = || format!("names no test title after {file}");
    let arrow = arrow.ok_or_else(no_title)?;
    let (mut describes, last) = parse_titles(&rest[arrow..])?;
    let titles = match last {
        Last::Set(titles) => titles,
        Last::Title => vec![describes.pop().ok_or_else(no_title)?],
    };
    if describes.iter().chain(&titles).any(|t| trim(t).is_empty()) {
        return Err("has an empty title".into());
    }
    if let Some(twice) = titles.iter().enumerate().find(|(k, t)| titles[..*k].contains(t)).map(|(_, t)| t) {
        return Err(format!("names \"{twice}\" twice in its set of titles"));
    }
    Ok(Citation { file: clean, describes, titles })
}

/// The cited file in canonical form, which must be a path relative to the project root.
pub(super) fn cited_file(file: &str) -> Result<String, String> {
    if file.is_empty() {
        return Err("names no file".into());
    }
    if file.contains(['"', '\'']) {
        return Err("has no \" > \" between the file and the title".into());
    }
    let clean = posix_normalize(&normalise(file));
    if clean.starts_with('/') || is_drive(&clean) || clean.starts_with("../") {
        return Err(format!("names {file}, which is not a path relative to the project root"));
    }
    Ok(clean)
}

fn is_drive(path: &str) -> bool {
    let mut chars = path.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':')
}

/// `/^test\s+[^\s"']+\s*>\s*["']/` on the text after `test`.
fn near_miss(after: &str) -> bool {
    let spaced = after.trim_start_matches(is_js_space);
    if spaced.len() == after.len() {
        return false;
    }
    let end = spaced.find(|c: char| is_js_space(c) || c == '"' || c == '\'').unwrap_or(spaced.len());
    if end == 0 {
        return false;
    }
    // `[^\s"']+` is greedy but may give back characters for `\s*>`: any `>` inside the run
    // can end it.
    let run = &spaced[..end];
    let tail = &spaced[end..];
    let quoted_after_arrow = |s: &str| {
        s.trim_start_matches(is_js_space)
            .strip_prefix('>')
            .is_some_and(|r| r.trim_start_matches(is_js_space).starts_with(['"', '\'']))
    };
    if quoted_after_arrow(tail) {
        return true;
    }
    run.char_indices().any(|(k, c)| k > 0 && c == '>' && quoted_after_arrow(&spaced[k..]))
}

/// How the titles of a citation end.
enum Last {
    /// With a quoted title: the last of the titles read.
    Title,
    /// With a set of titles, after the ones read.
    Set(Vec<String>),
}

/// Quoted ` > "…"` segments up to the end, the last of which may be a set; Err says what is
/// wrong instead.
fn parse_titles(text: &str) -> Result<(Vec<String>, Last), String> {
    let mut titles = Vec::new();
    let mut at = 0;
    while let Some((title, end)) = segment(text, at) {
        titles.push(title);
        at = end;
    }
    if let Some(set) = set_segment(text, at) {
        let (names, end) = set?;
        if !trim(&text[end..]).is_empty() {
            return Err("has a set of titles that is not the last segment".into());
        }
        return Ok((titles, Last::Set(names)));
    }
    let left = trim(&text[at..]);
    if left.is_empty() {
        return Ok((titles, Last::Title));
    }
    let shown =
        if left.chars().count() > SHOWN { format!("{}…", prefix(left, SHOWN)) } else { left.to_string() };
    let unclosed =
        left.strip_prefix('>').is_some_and(|r| r.trim_start_matches(is_js_space).starts_with(['"', '\'']));
    if unclosed {
        Err(format!("has a title with no closing quote: {shown}"))
    } else {
        Err(format!("has {shown} where a quoted title belongs"))
    }
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `/\s*>\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')/y` at `at`: the unescaped title and
/// where the match ends.
fn segment(text: &str, at: usize) -> Option<(String, usize)> {
    let rest = text[at..].trim_start_matches(is_js_space);
    let rest = rest.strip_prefix('>')?.trim_start_matches(is_js_space);
    quoted(text, text.len() - rest.len())
}

/// ` > {"…", "…"}` at `at`: the unescaped titles and where the set ends. None when no set
/// opens there; Err when one opens and is not closed, or holds no title.
fn set_segment(text: &str, at: usize) -> Option<Result<(Vec<String>, usize), String>> {
    let rest = text[at..].trim_start_matches(is_js_space);
    let rest = rest.strip_prefix('>')?.trim_start_matches(is_js_space).strip_prefix('{')?;
    let mut titles = Vec::new();
    let mut at = text.len() - rest.len();
    loop {
        let rest = text[at..].trim_start_matches(is_js_space);
        let from = text.len() - rest.len();
        if let Some((title, end)) = quoted(text, from) {
            titles.push(title);
            at = end;
        }
        let rest = text[at..].trim_start_matches(is_js_space);
        let next = text.len() - rest.len() + 1;
        match rest.chars().next() {
            Some('}') if titles.is_empty() => return Some(Err("has an empty set of titles".into())),
            Some('}') => return Some(Ok((titles, next))),
            Some(',') if at > from => at = next,
            _ => {
                return Some(Err(
                    "has a set of titles that is not closed: write {\"<title>\", \"<title>\"}".into()
                ));
            }
        }
    }
}

/// The quoted title that starts at `start`, unescaped, and where its closing quote ends.
fn quoted(text: &str, start: usize) -> Option<(String, usize)> {
    let quote = text[start..].chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let start = start + 1;
    let mut title = String::new();
    let mut chars = text[start..].char_indices();
    while let Some((k, c)) = chars.next() {
        if c == quote {
            return Some((title, start + k + 1));
        }
        if c == '\\' {
            // `\\.`: an escape takes any character but a line terminator.
            match chars.next() {
                Some((_, next)) if !is_line_terminator(next) => title.push(next),
                _ => return None,
            }
        } else {
            title.push(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestations::directives::parse_directive;

    fn message(body: &str) -> String {
        match parse_directive(body, 4) {
            Some(Directive::Invalid { message, .. }) => message,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn citations_parse() {
        assert_eq!(
            parse_directive(" test: test/a.test.ts > \"throws\" ", 1),
            Some(Directive::Test(Citation {
                file: "test/a.test.ts".into(),
                describes: vec![],
                titles: vec!["throws".into()]
            }))
        );
        assert_eq!(
            parse_directive("test: ./test/a.test.ts > 'it\\'s fine'", 1),
            Some(Directive::Test(Citation {
                file: "test/a.test.ts".into(),
                describes: vec![],
                titles: vec!["it's fine".into()]
            }))
        );
        assert_eq!(
            parse_directive("test: a.test.ts > \"retry\" > \"with backoff\" > \"waits\"", 1),
            Some(Directive::Test(Citation {
                file: "a.test.ts".into(),
                describes: vec!["retry".into(), "with backoff".into()],
                titles: vec!["waits".into()]
            }))
        );
        assert_eq!(parse_directive(" testing notes: revisit ", 1), None);
        assert_eq!(parse_directive(" test this paragraph again later ", 1), None);
        assert_eq!(parse_directive(" tests: none yet ", 1), None);
    }

    #[test]
    fn malformed_citations() {
        assert!(message("test: a.test.ts").contains("test at L4 names no test title after a.test.ts"));
        assert!(message("test:").contains("names no file"));
        assert!(message("test: a.test.ts \"x\"").contains("no \" > \" between the file and the title"));
        assert!(message("test: a.test.ts > x").contains("where a quoted title belongs"));
        assert!(message("test: a.test.ts > \"x").contains("no closing quote"));
        assert!(message("test: a.test.ts > \"\"").contains("empty title"));
        assert!(message("test: a.test.ts > \"x\" trailing").contains("where a quoted title belongs"));
        assert!(message("test: /abs/a.test.ts > \"x\"").contains("not a path relative"));
        assert!(message("test: ../a.test.ts > \"x\"").contains("not a path relative"));
        assert!(message("test a.test.ts > \"x\"").contains("no colon after \"test\""));
    }

    #[test]
    fn a_set_of_titles_cites_each_of_them() {
        assert_eq!(
            parse_directive("test: agent/launch_test.go > \"TestRun\" > {\"python\", 'node',\n \"ruby\"}", 1),
            Some(Directive::Test(Citation {
                file: "agent/launch_test.go".into(),
                describes: vec!["TestRun".into()],
                titles: vec!["python".into(), "node".into(), "ruby".into()]
            }))
        );
        let top = parse_directive("test: a.test.ts > {\"adds\"}", 1);
        assert!(matches!(top, Some(Directive::Test(c)) if c.describes.is_empty() && c.titles == ["adds"]));
    }

    #[test]
    fn a_malformed_set_of_titles_is_an_error() {
        assert!(message("test: a.go > \"T\" > {}").contains("test at L4 has an empty set of titles"));
        assert!(
            message("test: a.go > \"T\" > {\"a\", \"b\"").contains("has a set of titles that is not closed")
        );
        assert!(
            message("test: a.go > \"T\" > {\"a\" \"b\"}").contains("has a set of titles that is not closed")
        );
        assert!(
            message("test: a.go > {\"a\"} > \"b\"")
                .contains("has a set of titles that is not the last segment")
        );
        assert!(message("test: a.go > \"T\" > {\"a\", \"\"}").contains("has an empty title"));
        assert!(
            message("test: a.go > \"T\" > {\"a\", \"a\"}").contains("names \"a\" twice in its set of titles")
        );
    }
}
