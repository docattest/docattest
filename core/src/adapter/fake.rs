//! A stand-in for the TypeScript adapter, for unit tests only. The core holds no language's
//! rules; the tests still need a language to answer, and no adapter process runs in them.
//! Everything here mirrors what `docattest-adapter-typescript` (`adapters/typescript/src/language/` in
//! the repository) answers, in as few lines as the tests need.

use std::collections::{HashMap, HashSet};

use serde_json::json;

use super::{DocProse, ExpressionShape, Language, SourceFiles};
use crate::api::attestation::{Attestation, example_from_adapter};
use crate::api::errors::Result;
use crate::markdown::text::prose;
use crate::text::{is_js_space, is_word};

/// What the TypeScript adapter says of its language, and of one project's code.
pub struct Typescript {
    block_languages: Vec<String>,
    source_files: SourceFiles,
    /// The open project: its files, and the files its surfaces are read from.
    code: HashMap<String, String>,
    surface_files: Vec<String>,
}

/// The stand-in, with no project open.
pub fn typescript() -> Typescript {
    let block_languages = ["ts", "typescript", "tsx", "mts", "cts", "js", "javascript", "jsx", "mjs", "cjs"];
    Typescript {
        block_languages: block_languages.iter().map(|s| s.to_string()).collect(),
        source_files: super::sources::tests::typescript(),
        code: HashMap::new(),
        surface_files: Vec::new(),
    }
}

impl Typescript {
    /// The stand-in with a session over `files`, whose surfaces are read from `surface_files`.
    pub fn open(files: &[(String, String)], surface_files: &[&str]) -> Typescript {
        Typescript {
            code: files.iter().cloned().collect(),
            surface_files: surface_files.iter().map(|s| s.to_string()).collect(),
            ..typescript()
        }
    }
}

impl Language for Typescript {
    fn name(&self) -> &str {
        "typescript"
    }

    fn reads_tests(&self) -> bool {
        true
    }

    fn block_languages(&self) -> &[String] {
        &self.block_languages
    }

    fn info_words(&self) -> &[String] {
        &[]
    }

    fn source_files(&self) -> &SourceFiles {
        &self.source_files
    }

    /// One example per file that holds `@example`, at its first doc comment, defending its
    /// description.
    fn doc_comments(&self, files: &[(&str, &str)]) -> Result<Vec<Attestation>> {
        let mut out = Vec::new();
        for (page, text) in files {
            if !text.contains("@example") {
                continue;
            }
            let Some(unit) = doc_comments(text).into_iter().next() else { continue };
            let example = json!({
                "id": format!("{page}:{}:example:{}", unit.start, out.len()),
                "page": page, "line": unit.start, "kind": "example", "claim": unit.text,
                "lang": "ts", "mode": "check", "network": false,
                "code": "assert.equal(double(2), 4);", "weak": false, "owner": "double",
            });
            out.extend(example_from_adapter(&example));
        }
        Ok(out)
    }

    fn assertions(&self, examples: &[(&str, &str)]) -> Result<Vec<bool>> {
        Ok(examples.iter().map(|(_, code)| asserts(code)).collect())
    }

    fn doc_prose(&self, files: &[(&str, &str)]) -> Result<Vec<Vec<DocProse>>> {
        Ok(files
            .iter()
            .map(
                |(page, text)| if self.source_files.is_match(page) { doc_comments(text) } else { Vec::new() },
            )
            .collect())
    }

    fn mentions(&self, spans: &[&str], symbols: &[&str]) -> Result<Vec<Option<String>>> {
        Ok(spans
            .iter()
            .map(|span| {
                leading(span.trim_matches(is_js_space)).filter(|s| symbols.contains(s)).map(str::to_string)
            })
            .collect())
    }

    fn expressions(&self, expressions: &[&str]) -> Result<Vec<ExpressionShape>> {
        Ok(expressions
            .iter()
            .map(|expression| {
                let tokens = tokens(expression);
                let shape: Vec<&str> = tokens.iter().map(|(t, name)| if *name { "·" } else { t }).collect();
                ExpressionShape {
                    shape: shape.join(" "),
                    names: tokens.iter().filter(|(_, name)| *name).map(|(t, _)| t.to_string()).collect(),
                }
            })
            .collect())
    }

    fn in_code(&self, names: &[&str], files: &[String]) -> Result<Vec<String>> {
        let mut words: HashSet<&str> = HashSet::new();
        for file in files.iter().chain(&self.surface_files) {
            let Some(text) = self.code.get(file) else { continue };
            words.extend(tokens(text).into_iter().filter(|(_, name)| *name).map(|(t, _)| t));
        }
        Ok(names.iter().filter(|n| words.contains(*n)).map(|n| n.to_string()).collect())
    }
}

/// `/\bassert\s*[.(]/`.
fn asserts(code: &str) -> bool {
    code.match_indices("assert").any(|(at, _)| {
        let boundary = !code[..at].chars().next_back().is_some_and(is_word);
        boundary && code[at + 6..].trim_start_matches(is_js_space).starts_with(['.', '('])
    })
}

/// The matches of `/\/\*\*[\s\S]*?\*\//g`, each as its description (the prose before the
/// first `@tag` line) and its lines.
fn doc_comments(text: &str) -> Vec<DocProse> {
    let mut units = Vec::new();
    let mut at = 0;
    while let Some(open) = text[at..].find("/**").map(|k| at + k) {
        let Some(close) = text[open + 3..].find("*/").map(|k| open + 3 + k) else { break };
        let comment = &text[open..close + 2];
        let start = text[..open].matches('\n').count() + 1;
        let inner = &comment[3..comment.len() - 2];
        let lines: Vec<&str> = inner
            .split('\n')
            .map(|line| match line.trim_start_matches(is_js_space).strip_prefix('*') {
                Some(rest) => rest.strip_prefix(' ').unwrap_or(rest),
                None => line,
            })
            .take_while(|line| {
                let mut rest = line.trim_start_matches(is_js_space).chars();
                !(rest.next() == Some('@') && rest.next().is_some_and(is_word))
            })
            .collect();
        units.push(DocProse {
            text: prose(&lines.join(" ")),
            start,
            end: start + comment.matches('\n').count(),
        });
        at = close + 2;
    }
    units
}

fn starts_identifier(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn continues_identifier(c: char) -> bool {
    is_word(c) || c == '$'
}

/// The symbol a code span starts with: `retry(fn)`, `new Client()`, `RETRY.max`; not
/// `retry-after` or `retry.ts`.
fn leading(code: &str) -> Option<&str> {
    let after_keyword = ["new", "await"].iter().find_map(|keyword| {
        let rest = code.strip_prefix(keyword)?;
        let spaced = rest.trim_start_matches(is_js_space);
        (spaced.len() < rest.len()).then_some(spaced)
    });
    after_keyword.and_then(identifier).or_else(|| identifier(code))
}

fn identifier(text: &str) -> Option<&str> {
    if !text.chars().next().is_some_and(starts_identifier) {
        return None;
    }
    let end = text.find(|c: char| !continues_identifier(c)).unwrap_or(text.len());
    let (name, rest) = text.split_at(end);
    let file = rest.strip_prefix('.').is_some_and(|extension| {
        let script = extension.strip_prefix(['m', 'c']).unwrap_or(extension);
        matches!(extension, "json" | "md" | "yml" | "yaml")
            || matches!(script.strip_suffix('x').unwrap_or(script), "js" | "ts")
    });
    (!rest.starts_with(['-', '/', '@', '\'', '"']) && !file).then_some(name)
}

/// Identifiers, numbers, quoted strings and single characters, and whether each is an
/// identifier.
fn tokens(text: &str) -> Vec<(&str, bool)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        let run = |keep: fn(char) -> bool| rest.find(|c| !keep(c)).unwrap_or(rest.len());
        let (length, name) = if starts_identifier(first) {
            (run(continues_identifier), true)
        } else if first.is_ascii_digit() {
            (run(|c| c.is_ascii_digit()), false)
        } else if matches!(first, '\'' | '"' | '`') {
            (rest[1..].find(first).map_or(1, |close| close + 2), false)
        } else if is_js_space(first) {
            rest = &rest[first.len_utf8()..];
            continue;
        } else {
            (first.len_utf8(), false)
        };
        out.push((&rest[..length], name));
        rest = &rest[length..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stand_in_answers_as_the_typescript_adapter_does() {
        let ts =
            Typescript::open(&[("src/a.ts".into(), "export const LIMIT = 3; // MAX".into())], &["src/a.ts"]);
        assert_eq!(ts.assertions(&[("ts", "assert.ok(1)"), ("ts", "reassert(1)")]).unwrap(), [true, false]);
        let text = "// x\n/**\n * Doubles a number.\n *   Twice.\n * @example\n * // check\n */\n/** One. */";
        let prose = ts.doc_prose(&[("src/a.ts", text), ("docs/a.md", text)]).unwrap();
        assert_eq!(
            prose[0],
            vec![
                DocProse { text: "Doubles a number. Twice.".into(), start: 2, end: 7 },
                DocProse { text: "One.".into(), start: 8, end: 8 },
            ]
        );
        assert!(prose[1].is_empty());
        let spans = ["retry(fn)", " new Client()", "RETRY.max", "retry-after", "retry.ts", "parse"];
        let named = ts.mentions(&spans, &["retry", "Client", "RETRY"]).unwrap();
        let named: Vec<Option<&str>> = named.iter().map(Option::as_deref).collect();
        assert_eq!(named, [Some("retry"), Some("Client"), Some("RETRY"), None, None, None]);
        let shapes = ts.expressions(&["Math.min(LIMIT, 3)", "f('a b')"]).unwrap();
        assert_eq!(shapes[0].shape, "· . · ( · , 3 )");
        assert_eq!(shapes[0].names, ["Math", "min", "LIMIT"]);
        assert_eq!(shapes[1].shape, "· ( 'a b' )");
        assert_eq!(ts.in_code(&["LIMIT", "MAX", "GONE"], &[]).unwrap(), ["LIMIT", "MAX"]);
    }
}
