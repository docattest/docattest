//! Fenced blocks as examples.

use crate::api::attestation::{Attestation, Body, Example, Mode};
use crate::text::{is_js_space, is_word};

/// What the planner knows about the languages and renderer tags of a project.
pub struct ExampleOptions<'a> {
    /// Info-string languages the adapter runs, lower case.
    pub languages: &'a [String],
    /// Renderer words accepted in an info string and otherwise ignored, e.g. `twoslash`.
    pub ignore_tags: &'a [String],
    /// Words of the adapter's own (`hello.infoWords`): accepted, and passed on with the
    /// example. An entry `key=` names an attribute that is passed on too.
    pub info_words: &'a [String],
}

/// One piece of an info string: a word, or an attribute `key=value` (the value as written
/// between its quotes, when it has them).
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Piece<'a> {
    Word(&'a str),
    Attribute(&'a str, &'a str),
}

/// The pieces of an info string, in order. An attribute is a key of ASCII letters, digits,
/// `_` and `-`, then `=`, then a value: quoted with `"` or `'` (it may then hold spaces), or
/// everything up to the next space. Renderers write them (`title="a.py" hl_lines="1 2"`).
pub(super) fn pieces(info: &str) -> Vec<Piece<'_>> {
    let key_char = |c: char| is_word(c) || c == '-';
    let mut out = Vec::new();
    let mut rest = info;
    loop {
        rest = rest.trim_start_matches(is_js_space);
        if rest.is_empty() {
            return out;
        }
        let key = rest.find(|c| !key_char(c)).unwrap_or(rest.len());
        let word = rest.find(is_js_space).unwrap_or(rest.len());
        if key == 0 || !rest[key..].starts_with('=') {
            out.push(Piece::Word(&rest[..word]));
            rest = &rest[word..];
            continue;
        }
        let value = &rest[key + 1..];
        let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''));
        let (text, after) = match quote {
            Some(quote) => match value[1..].find(quote) {
                Some(end) => (&value[1..1 + end], &value[end + 2..]),
                None => (&value[1..], ""),
            },
            None => {
                let end = value.find(is_js_space).unwrap_or(value.len());
                (&value[..end], &value[end..])
            }
        };
        out.push(Piece::Attribute(&rest[..key], text));
        rest = after;
    }
}

const TAGS: [&str; 3] = ["check", "compile", "network"];

/// The planned examples whose weakness is still to be decided: every `check` block, as its
/// block language and code. What an assertion looks like is the language's to say
/// (`assertions`), so they are asked about together; [`mark_weak`] takes the answers.
pub fn checks(attestations: &[Attestation]) -> Vec<(&str, &str)> {
    attestations
        .iter()
        .filter_map(|a| match &a.body {
            Body::Example(e) if e.mode == Mode::Check => Some((e.lang.as_str(), e.code.as_str())),
            _ => None,
        })
        .collect()
}

/// Decide which planned examples are weak: a `check` block that asserts nothing runs, and
/// proves nothing. `asserting` answers [`checks`], in order; a missing answer is weak.
pub fn mark_weak(attestations: &mut [Attestation], asserting: &[bool]) {
    let mut answers = asserting.iter();
    for a in attestations {
        if let Body::Example(e) = &mut a.body
            && e.mode == Mode::Check
        {
            e.weak = answers.next() != Some(&true);
        }
    }
}

/// `info.replace(/\[[^\]]*\]/g, ' ')`: bracketed titles are renderer tab labels.
pub(super) fn without_titles(info: &str) -> String {
    let mut out = String::with_capacity(info.len());
    let mut rest = info;
    while let Some(open) = rest.find('[') {
        match rest[open + 1..].find(']') {
            Some(close) => {
                out.push_str(&rest[..open]);
                out.push(' ');
                rest = &rest[open + 1 + close + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// The example a fenced block declares, or None for documentation-only blocks; with the
/// planning error, if any. Untagged blocks are documentation, unless an unknown tag sits on a
/// block in a language the adapter runs: that is a typo of check or compile until proven
/// otherwise, so it fails closed. `weak` is left for [`mark_weak`].
pub fn plan_example(
    line: usize,
    info: &str,
    code: &str,
    options: &ExampleOptions,
) -> Option<(Example, Option<String>)> {
    let cleaned = without_titles(info);
    let read = InfoString::read(&cleaned, options);
    let mode = read.mode();
    if mode.is_none() && !(read.runnable && !read.unknown.is_empty()) {
        return None;
    }
    let error = read.error(line);
    let example = Example {
        lang: read.lang.to_string(),
        mode: mode.unwrap_or(Mode::Compile),
        network: read.has("network"),
        code: code.to_string(),
        weak: false,
        owner: None,
        tags: read.passed,
    };
    Some((example, error))
}

/// What a block's info string says, read against the project's options.
struct InfoString<'a> {
    lang: &'a str,
    /// The words after the language.
    tags: Vec<&'a str>,
    /// The words and attributes the adapter declares, as they are passed on to it.
    passed: Vec<String>,
    /// Every tag the info string may hold.
    known: Vec<&'a str>,
    unknown: Vec<&'a str>,
    /// The adapter runs blocks in the language.
    runnable: bool,
}

impl<'a> InfoString<'a> {
    fn read(cleaned: &'a str, options: &ExampleOptions<'a>) -> InfoString<'a> {
        let all = pieces(cleaned);
        // The first piece is the language, whatever it looks like; attributes are never tags.
        let lang = match all.first() {
            Some(Piece::Word(lang)) => *lang,
            _ => "",
        };
        let after = all.get(1..).unwrap_or_default();
        let tags = tag_words(after);
        let passed = passed_on(after, options.info_words);
        let own = options.info_words.iter().map(String::as_str).filter(|w| !w.ends_with('='));
        let known: Vec<&str> =
            TAGS.iter().copied().chain(options.ignore_tags.iter().map(String::as_str)).chain(own).collect();
        let unknown: Vec<&str> = tags.iter().copied().filter(|t| !known.contains(t)).collect();
        let runnable = options.languages.iter().any(|l| *l == lang.to_lowercase());
        InfoString { lang, tags, passed, known, unknown, runnable }
    }

    fn has(&self, tag: &str) -> bool {
        self.tags.contains(&tag)
    }

    fn mode(&self) -> Option<Mode> {
        if self.has("check") {
            Some(Mode::Check)
        } else if self.has("compile") {
            Some(Mode::Compile)
        } else {
            None
        }
    }

    fn error(&self, line: usize) -> Option<String> {
        let at = format!("example at L{line}");
        if self.has("check") && self.has("compile") {
            Some(format!("{at} is tagged both check and compile"))
        } else if let Some(first) = self.unknown.first() {
            Some(unknown_tag(&at, first, &self.known))
        } else if !self.runnable {
            let shown = if self.lang.is_empty() { "(no language)" } else { self.lang };
            Some(format!("{at}: no adapter runs \"{shown}\" blocks"))
        } else if self.has("network") && self.mode() != Some(Mode::Check) {
            Some(format!("{at}: network applies to check blocks only"))
        } else {
            None
        }
    }
}

/// The words among an info string's pieces after its language: its tags.
pub(super) fn tag_words<'a>(after: &[Piece<'a>]) -> Vec<&'a str> {
    after
        .iter()
        .filter_map(|piece| match piece {
            Piece::Word(word) => Some(*word),
            Piece::Attribute(..) => None,
        })
        .collect()
}

/// The error of a tag that is not `known`: a typo of `check` until proven otherwise, or a
/// renderer's word that the config should name.
pub(super) fn unknown_tag(at: &str, unknown: &str, known: &[&str]) -> String {
    let known = format!("known: {}; add renderer tags to \"ignoreTags\"", known.join(", "));
    format!("{at}: unknown tag \"{unknown}\" ({known})")
}

/// The pieces the adapter declares (`infoWords`), as it is given them.
fn passed_on(pieces: &[Piece<'_>], info_words: &[String]) -> Vec<String> {
    let declared = |word: &str| info_words.iter().any(|w| w == word);
    pieces
        .iter()
        .filter_map(|piece| match piece {
            Piece::Word(word) => declared(word).then(|| (*word).to_string()),
            Piece::Attribute(key, value) => declared(&format!("{key}=")).then(|| format!("{key}={value}")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_says_which_check_blocks_assert() {
        let languages = vec!["ts".to_string()];
        let options = ExampleOptions { languages: &languages, ignore_tags: &[], info_words: &[] };
        let page = "```ts check\nassert.ok(1)\n```\n\n```ts compile\nx\n```\n\n```ts check\nlog(1)\n```\n";
        let mut planned = crate::attestations::plan::plan_page("p.md", page, &options).unwrap().attestations;
        assert_eq!(checks(&planned), [("ts", "assert.ok(1)"), ("ts", "log(1)")]);
        let weak = |planned: &[Attestation]| -> Vec<bool> {
            planned.iter().map(|a| matches!(&a.body, Body::Example(e) if e.weak)).collect()
        };
        mark_weak(&mut planned, &[true, false]);
        assert_eq!(weak(&planned), [false, false, true]);
        // An answer that is missing leaves the block weak: it fails closed.
        mark_weak(&mut planned, &[]);
        assert_eq!(weak(&planned), [true, false, true]);
    }

    #[test]
    fn tags() {
        let languages = vec!["ts".to_string()];
        let ignore = vec!["twoslash".to_string()];
        let options = ExampleOptions { languages: &languages, ignore_tags: &ignore, info_words: &[] };
        assert!(plan_example(1, "ts", "x", &options).is_none());
        assert!(plan_example(1, "sh", "x", &options).is_none());
        let (e, err) = plan_example(1, "ts twoslash compile [Tab (A)]", "x", &options).unwrap();
        assert_eq!((e.mode, err), (Mode::Compile, None));
        let (_, err) = plan_example(3, "ts chekc", "x", &options).unwrap();
        assert!(err.unwrap().contains("unknown tag \"chekc\""));
    }

    #[test]
    fn an_info_string_is_words_and_attributes() {
        use Piece::{Attribute, Word};
        assert_eq!(
            pieces("python title=\"a b.py\" hl_lines='1 2' check linenums=1  x="),
            [
                Word("python"),
                Attribute("title", "a b.py"),
                Attribute("hl_lines", "1 2"),
                Word("check"),
                Attribute("linenums", "1"),
                Attribute("x", "")
            ]
        );
        assert_eq!(pieces("a=\"b\"c d=\"e"), [Attribute("a", "b"), Word("c"), Attribute("d", "e")]);
        assert_eq!(pieces("=x a==b é=1 c++"), [Word("=x"), Attribute("a", "=b"), Word("é=1"), Word("c++")]);
        assert_eq!(pieces(" \t "), []);
    }

    #[test]
    fn attributes_are_ignored_and_the_adapters_words_are_passed_on() {
        let languages = vec!["python".to_string()];
        let own = vec!["continue".to_string(), "session=".to_string()];
        let options = ExampleOptions { languages: &languages, ignore_tags: &[], info_words: &own };
        // A renderer's attributes are no tags: the block is documentation, or runs as tagged.
        assert!(plan_example(1, "python title=\"x.py\" hl_lines=\"1 2\"", "x", &options).is_none());
        let (e, err) =
            plan_example(1, "python title=\"x.py\" hl_lines=\"1 2\" check", "x", &options).unwrap();
        assert_eq!((e.mode, err, e.tags.len()), (Mode::Check, None, 0));
        // The adapter's own words and attributes reach it, in the order written.
        let (e, err) = plan_example(1, "python session='a b' check continue title=x", "x", &options).unwrap();
        assert_eq!((err, e.tags), (None, vec!["session=a b".to_string(), "continue".to_string()]));
        // A word of the adapter's is no mode: without check or compile the block is prose.
        assert!(plan_example(1, "python continue", "x", &options).is_none());
        let (_, err) = plan_example(1, "python continue chekc", "x", &options).unwrap();
        assert!(err.unwrap().contains("known: check, compile, network, continue;"));
        // Nothing is the adapter's unless it says so.
        let plain = ExampleOptions { languages: &languages, ignore_tags: &[], info_words: &[] };
        assert!(plan_example(1, "python continue", "x", &plain).unwrap().1.unwrap().contains("unknown tag"));
    }
}
