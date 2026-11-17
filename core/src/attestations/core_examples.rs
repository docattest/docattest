//! Blocks the core runs itself, whatever the project's language: `console check`
//! transcripts and `md check` sample pages. They are reported as examples, and no adapter is
//! asked about them.

use super::examples::{Piece, pieces, tag_words, unknown_tag, without_titles};
use super::tags::{UnknownWord, unknown_to_report};
use crate::api::attestation::{Body, Console, Sample};
use crate::console::transcript::{PROMPT, TranscriptError, parse_transcript};

/// Where a sample page is written in the copy of its fixture, unless `as=` says otherwise.
const DEFAULT_SAMPLE_PATH: &str = "docs/sample.md";

/// The block languages the core runs, lower case.
const CONSOLE: &str = "console";
const MARKDOWN: [&str; 2] = ["md", "markdown"];

/// The body a block the core runs declares, with its planning error, if any; None for any
/// other block. An untagged console or Markdown block is documentation, as any untagged
/// block is, unless a misspelling of a tag sits on it: then it fails closed.
pub fn plan_core_example(
    line: usize,
    info: &str,
    code: &str,
    ignore_tags: &[String],
) -> Option<(Body, Option<String>)> {
    let cleaned = without_titles(info);
    let all = pieces(&cleaned);
    let Some((Piece::Word(lang), after)) = all.split_first() else { return None };
    let lower = lang.to_lowercase();
    if lower != CONSOLE && !MARKDOWN.contains(&lower.as_str()) {
        return None;
    }
    let tags = Tags::read(after, ignore_tags);
    let misspelled = tags.unknown.as_ref().is_some_and(|unknown| unknown.meant.is_some());
    if !tags.has(CHECK) && !tags.has(COMPILE) && !misspelled {
        return None;
    }
    let at = format!("{lang} block at L{line}");
    let (body, body_error) = if lower == CONSOLE {
        console(&at, code, tags.attribute("cwd"))
    } else {
        sample(&at, lang, code, &tags)
    };
    Some((body, tags.error(&at).or(body_error)))
}

fn console(at: &str, code: &str, cwd: Option<&str>) -> (Body, Option<String>) {
    let (steps, error) = match parse_transcript(code) {
        Ok(steps) => (steps, None),
        Err(TranscriptError::NoCommand) => {
            (Vec::new(), Some(format!("{at} has no command; a command starts with \"{PROMPT}\"")))
        }
        Err(TranscriptError::NotACommand(n)) => (
            Vec::new(),
            Some(format!("{at}: block line {n} is not a command; a command starts with \"{PROMPT}\"")),
        ),
    };
    (Body::Console(Console { code: code.to_string(), cwd: cwd.map(str::to_string), steps }), error)
}

fn sample(at: &str, lang: &str, code: &str, tags: &Tags) -> (Body, Option<String>) {
    let fixture = tags.attribute("fixture").unwrap_or_default();
    let path = tags.attribute("as").unwrap_or(DEFAULT_SAMPLE_PATH);
    let error = if fixture.is_empty() {
        Some(format!("{at} needs fixture=<dir>: the project the sample page is checked in"))
    } else if !is_inside(path) {
        Some(format!("{at}: as={path} must be a relative path inside the fixture"))
    } else {
        None
    };
    let body = Body::Sample(Sample {
        lang: lang.to_string(),
        code: code.to_string(),
        fixture: fixture.to_string(),
        path: path.to_string(),
    });
    (body, error)
}

/// A relative path that stays below the directory it is relative to.
fn is_inside(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && path.split('/').all(|part| !part.is_empty() && part != "..")
}

/// The one tag a block the core runs takes.
const CHECK: &str = "check";
const COMPILE: &str = "compile";
/// The tags of examples in the project's language, which do not apply here: known, so that
/// the error says so.
const NOT_HERE: [&str; 2] = [COMPILE, "network"];

/// The words and attributes after a block's language.
struct Tags<'a> {
    words: Vec<&'a str>,
    attributes: Vec<(&'a str, &'a str)>,
    /// Renderer words the config accepts.
    ignore_tags: &'a [String],
    /// The word that is no tag here which the error names, if any.
    unknown: Option<UnknownWord<'a>>,
}

impl<'a> Tags<'a> {
    fn read(after: &[Piece<'a>], ignore_tags: &'a [String]) -> Tags<'a> {
        let attributes = after
            .iter()
            .filter_map(|piece| match piece {
                Piece::Attribute(key, value) => Some((*key, *value)),
                Piece::Word(_) => None,
            })
            .collect();
        let words = tag_words(after);
        let known =
            |word: &str| word == CHECK || NOT_HERE.contains(&word) || ignore_tags.iter().any(|t| t == word);
        let unknown = unknown_to_report(words.iter().copied().filter(|word| !known(word)));
        Tags { words, attributes, ignore_tags, unknown }
    }

    fn has(&self, tag: &str) -> bool {
        self.words.contains(&tag)
    }

    /// The value of the last attribute `key`, as a renderer reads it.
    fn attribute(&self, key: &str) -> Option<&'a str> {
        self.attributes.iter().rev().find(|(k, _)| *k == key).map(|(_, value)| *value)
    }

    fn error(&self, at: &str) -> Option<String> {
        if self.has(COMPILE) {
            Some(format!("{at} cannot be compiled: it runs, tagged {CHECK}"))
        } else if let Some(unknown) = &self.unknown {
            let known: Vec<&str> =
                std::iter::once(CHECK).chain(self.ignore_tags.iter().map(String::as_str)).collect();
            Some(unknown_tag(at, unknown, &known))
        } else if self.has("network") {
            Some(format!("{at}: network applies to examples in the project's language only"))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::transcript::Step;

    fn plan(info: &str, code: &str) -> Option<(Body, Option<String>)> {
        plan_core_example(4, info, code, &["twoslash".to_string()])
    }

    fn error_of(info: &str, code: &str) -> Option<String> {
        plan(info, code).and_then(|(_, error)| error)
    }

    #[test]
    fn a_console_block_is_planned_with_its_commands_and_directory() {
        let (body, error) = plan("console check cwd=tools title=\"x\" [Shell]", "$ echo a\na").unwrap();
        let step = Step { line: 1, command: "echo a".into(), output: vec!["a".into()], exit: 0 };
        let expected = Console { code: "$ echo a\na".into(), cwd: Some("tools".into()), steps: vec![step] };
        assert_eq!((body, error), (Body::Console(expected), None));
        assert_eq!(
            error_of("console check", "a\n$ b"),
            Some("console block at L4: block line 1 is not a command; a command starts with \"$ \"".into())
        );
        assert_eq!(
            error_of("Console check", ""),
            Some("Console block at L4 has no command; a command starts with \"$ \"".into())
        );
    }

    #[test]
    fn untagged_blocks_are_documentation_and_typos_fail_closed() {
        assert!(plan("console", "$ x").is_none());
        assert!(plan("md twoslash title=\"a\"", "x").is_none());
        assert!(plan("console vue", "$ x").is_none());
        assert!(plan("ts check", "x").is_none());
        assert!(plan("", "x").is_none());
        assert_eq!(
            error_of("console chekc", "$ x"),
            Some("console block at L4: unknown tag \"chekc\"; did you mean \"check\"?".into())
        );
        assert_eq!(
            error_of("console check vue", "$ x"),
            Some("console block at L4: unknown tag \"vue\" (known: check, twoslash; add renderer tags to \"ignoreTags\")".into())
        );
        assert_eq!(
            error_of("console compile", "$ x"),
            Some("console block at L4 cannot be compiled: it runs, tagged check".into())
        );
        assert_eq!(
            error_of("console check network", "$ x"),
            Some("console block at L4: network applies to examples in the project's language only".into())
        );
    }

    #[test]
    fn a_sample_page_needs_a_fixture_and_stays_inside_it() {
        let (body, error) = plan("markdown check fixture=testdata/fixture", "# A").unwrap();
        let sample = |path: &str| Sample {
            lang: "markdown".into(),
            code: "# A".into(),
            fixture: "testdata/fixture".into(),
            path: path.into(),
        };
        assert_eq!((body, error), (Body::Sample(sample("docs/sample.md")), None));
        let (body, error) = plan("md check fixture=f as=README.md", "# A").unwrap();
        assert_eq!((error, matches!(body, Body::Sample(s) if s.path == "README.md")), (None, true));
        assert_eq!(
            error_of("md check", "x"),
            Some("md block at L4 needs fixture=<dir>: the project the sample page is checked in".into())
        );
        for outside in ["../x.md", "/x.md", "a//b.md", "docs/../../x.md"] {
            let info = format!("md check fixture=f as={outside}");
            assert_eq!(
                error_of(&info, "x"),
                Some(format!("md block at L4: as={outside} must be a relative path inside the fixture"))
            );
        }
    }
}
