//! Glob matching: a port of picomatch 4.0.7, called as `picomatch(globs, { dot: true })`
//! and `picomatch.scan(pattern).isGlob` on a POSIX platform. picomatch's semantics are those
//! of every glob docattest reads, in the config and from an adapter, whichever adapter
//! serves the project.
//!
//! The port keeps picomatch's behaviour, quirks included, in three stages:
//!
//! 1. `make_re` reproduces picomatch's parser and yields the very regular expression
//!    source picomatch hands to `new RegExp`. It works on UTF-16 code units, because
//!    that is what JavaScript string indexing sees.
//! 2. `translate` reads that source with the grammar V8 applies to a flagless `RegExp`
//!    (ECMAScript Annex B), rejects what V8 rejects (picomatch then falls back to a regex
//!    that never matches), and rewrites the rest for `fancy_regex`: JavaScript `.`, `\w`,
//!    `\s`, `\b`, escapes, quantified assertions and backreference semantics are spelled
//!    out explicitly.
//! 3. Matching runs on a transcoding of the path where every UTF-16 code unit is one
//!    `char` (a lone surrogate maps into plane 16, where no transcoded text can otherwise
//!    have a character), so `?`, `[^/]` and `.` consume code units exactly as in
//!    JavaScript.
//!
//! `tests/fixtures/glob-cases.json` (generated from picomatch by `gen-glob.ts`) checks
//! all of it differentially. Known differences, all needing regex syntax written into a
//! glob or a pathological pattern:
//!
//! - A lookbehind (`(?<=…)`, `(?<!…)`) whose body has no constant length: V8 runs it,
//!   fancy_regex cannot compile it, so the glob never matches.
//! - A backreference (`\1`, `\k<name>`) to a group inside a repeated group: JavaScript
//!   clears that group at each iteration, fancy_regex keeps its last value.
//! - Patterns on which picomatch never returns (a backslash then three or more backslashes
//!   ending the pattern) are an error here instead of a hang.
//! - V8 and engine limits on huge patterns: picomatch throws a RangeError past about 8,700
//!   unclosed `(`, and V8 throws at the first match on regexes it finds too large (tens of
//!   thousands of atoms, or about 1,900 globstars); here those compile and match. Regexes
//!   nested more than 64 groups deep, which fancy_regex refuses, never match; matching
//!   that exceeds fancy_regex's backtracking limit reports no match.

mod constants;
mod extglob;
mod make_re;
mod parse;
mod scan;
mod translate;
mod units;

#[cfg(test)]
mod tests;

use std::borrow::Cow;

use fancy_regex::Regex;

use make_re::make_re;
use scan::Scanner;
use translate::compile;
use units::{transcode, w};

/// A UTF-16 string, as JavaScript sees one.
type U = Vec<u16>;

const MAX_LENGTH: usize = 1024 * 64;
const EMPTY_PATTERN: &str = "Expected pattern to be a non-empty string";
const UNDEFINED_TYPE: &str = "Cannot set properties of undefined (setting 'type')";
const HANGS: &str = "Pattern never finishes compiling in picomatch 4.0.7, which loops forever on a backslash followed by three or more backslashes at the end of a pattern";
/// fancy_regex gives up after this many backtracking steps; JavaScript has no limit.
const BACKTRACK_LIMIT: usize = 50_000_000;
/// Where transcoded lone surrogates live (plane 16).
const SURROGATE_BASE: u32 = 0x10_0000;

/// A predicate for project-relative paths matching any of a list of globs (dotfiles match).
#[derive(Debug)]
pub struct GlobMatcher {
    patterns: Vec<Compiled>,
}

#[derive(Debug)]
struct Compiled {
    /// The pattern with one leading `./` removed; a path equal to it always matches.
    glob: String,
    /// `None` where picomatch's `RegExp` failed to compile and it uses `/$^/` instead.
    regex: Option<Regex>,
}

impl GlobMatcher {
    /// A matcher for any of `globs`, each without one leading `./`. `Err(message)` where
    /// picomatch would throw, with the message it throws.
    pub fn new(globs: &[String]) -> Result<GlobMatcher, String> {
        let mut patterns = Vec::with_capacity(globs.len());
        for raw in globs {
            let glob = raw.strip_prefix("./").unwrap_or(raw).to_string();
            if glob.is_empty() {
                return Err(EMPTY_PATTERN.to_string());
            }
            let source = make_re(&w(&glob))?;
            let regex = compile(&source);
            patterns.push(Compiled { glob, regex });
        }
        Ok(GlobMatcher { patterns })
    }

    /// Whether `path` matches any of the globs.
    pub fn is_match(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let mut haystack: Option<Cow<'_, str>> = None;
        for pattern in &self.patterns {
            if path == pattern.glob {
                return true;
            }
            if let Some(regex) = &pattern.regex {
                let text = haystack.get_or_insert_with(|| transcode(path));
                if regex.is_match(text).unwrap_or(false) {
                    return true;
                }
            }
        }
        false
    }
}

/// `picomatch.scan(pattern).isGlob`.
pub fn is_glob(pattern: &str) -> bool {
    Scanner::new(&w(pattern)).is_glob()
}
