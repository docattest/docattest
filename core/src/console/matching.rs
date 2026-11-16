//! Comparing what a command printed with what its console block shows.

use crate::json::json_string;

/// A line of a block's output that stands for any number of lines, none included.
pub const ANY_LINES: &str = "...";

/// Where a command's output first departs from the block. Lines are the output's, from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// The output's line is not the one the block shows.
    Differs { line: usize, expected: String, actual: String },
    /// The output ended where the block shows another line.
    Ended { line: usize, expected: String },
    /// The output goes on where the block has ended.
    Extra { line: usize, actual: String },
    /// After `...`, no line of the output after line `after` is the one the block shows.
    NotFound { after: usize, expected: String },
}

impl Mismatch {
    /// The mismatch in words, each line quoted as a JSON string so its spaces show.
    pub fn describe(&self) -> String {
        match self {
            Mismatch::Differs { line, expected, actual } => {
                format!(
                    "output line {line} differs: expected {}, got {}",
                    json_string(expected),
                    json_string(actual)
                )
            }
            Mismatch::Ended { line, expected } => {
                format!("the output ended before line {line}, {}", json_string(expected))
            }
            Mismatch::Extra { line, actual } => {
                format!("output line {line}, {}, is not in the block", json_string(actual))
            }
            Mismatch::NotFound { after: 0, expected } => {
                format!("no line of the output is {}", json_string(expected))
            }
            Mismatch::NotFound { after, expected } => {
                format!("no line of the output after line {after} is {}", json_string(expected))
            }
        }
    }
}

/// The first place `actual` departs from `expected`, or None when it matches. Lines are
/// compared as they are; a line `...` in `expected` matches any run of lines.
pub fn first_mismatch(expected: &[String], actual: &[String]) -> Option<Mismatch> {
    walk(expected, actual, Progress { block: 0, output: 0 }).err().map(|failed| failed.mismatch)
}

/// How many lines of the block and of the output are behind.
#[derive(Debug, Clone, Copy)]
struct Progress {
    block: usize,
    output: usize,
}

/// A mismatch, and how many lines of the block were matched before it.
struct Failed {
    mismatch: Mismatch,
    matched: usize,
}

/// Match from here, at `done`.
fn walk(expected: &[String], actual: &[String], done: Progress) -> Result<(), Failed> {
    let failed = |mismatch| Err(Failed { mismatch, matched: done.block });
    let line = done.output + 1;
    let Some((first, rest)) = expected.split_first() else {
        return match actual.first() {
            None => Ok(()),
            Some(extra) => failed(Mismatch::Extra { line, actual: extra.clone() }),
        };
    };
    if first == ANY_LINES {
        return after_any_lines(rest, actual, Progress { block: done.block + 1, ..done });
    }
    match actual.split_first() {
        None => failed(Mismatch::Ended { line, expected: first.clone() }),
        Some((next, more)) if next == first => {
            walk(rest, more, Progress { block: done.block + 1, output: done.output + 1 })
        }
        Some((next, _)) => failed(Mismatch::Differs { line, expected: first.clone(), actual: next.clone() }),
    }
}

/// `...` was met: the rest must match from one of the lines that follow. Every line that
/// could begin it is tried; when none does, the attempt that matched most of the block is
/// reported.
fn after_any_lines(rest: &[String], actual: &[String], done: Progress) -> Result<(), Failed> {
    let Some(anchor) = rest.first() else { return Ok(()) };
    if anchor == ANY_LINES {
        return after_any_lines(&rest[1..], actual, Progress { block: done.block + 1, ..done });
    }
    let mut best: Option<Failed> = None;
    for skipped in (0..actual.len()).filter(|&k| actual[k] == *anchor) {
        let from = Progress { output: done.output + skipped, ..done };
        let Err(failed) = walk(rest, &actual[skipped..], from) else { return Ok(()) };
        if best.as_ref().is_none_or(|b| failed.matched > b.matched) {
            best = Some(failed);
        }
    }
    let not_found = || Failed {
        mismatch: Mismatch::NotFound { after: done.output, expected: anchor.clone() },
        matched: done.block,
    };
    Err(best.unwrap_or_else(not_found))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    fn check(expected: &[&str], actual: &[&str]) -> Option<Mismatch> {
        first_mismatch(&owned(expected), &owned(actual))
    }

    #[test]
    fn lines_are_compared_in_order() {
        assert_eq!(check(&["a", "b"], &["a", "b"]), None);
        assert_eq!(
            check(&["a", "b"], &["a", "c"]),
            Some(Mismatch::Differs { line: 2, expected: "b".into(), actual: "c".into() })
        );
        assert_eq!(check(&["a", "b"], &["a"]), Some(Mismatch::Ended { line: 2, expected: "b".into() }));
        assert_eq!(check(&["a"], &["a", "b"]), Some(Mismatch::Extra { line: 2, actual: "b".into() }));
        assert_eq!(check(&[], &[]), None);
    }

    #[test]
    fn three_dots_match_any_run_of_lines() {
        assert_eq!(check(&["..."], &[]), None);
        assert_eq!(check(&["a", "...", "z"], &["a", "z"]), None);
        assert_eq!(check(&["a", "...", "...", "z"], &["a", "b", "c", "z"]), None);
        assert_eq!(check(&["...", "x", "y"], &["x", "q", "x", "y"]), None);
        assert_eq!(check(&["a", "..."], &["a", "b"]), None);
        assert_eq!(
            check(&["a", "...", "z"], &["a", "b"]),
            Some(Mismatch::NotFound { after: 1, expected: "z".into() })
        );
        // The attempt that matched most of the block is the one reported.
        assert_eq!(
            check(&["...", "x", "y", "z"], &["x", "y", "w", "x", "q"]),
            Some(Mismatch::Differs { line: 3, expected: "z".into(), actual: "w".into() })
        );
        assert_eq!(
            check(&["...", "x", "y", "z"], &["x", "q", "x", "y", "w"]),
            Some(Mismatch::Differs { line: 5, expected: "z".into(), actual: "w".into() })
        );
    }

    #[test]
    fn a_mismatch_quotes_the_lines() {
        let differs = Mismatch::Differs { line: 2, expected: "b ".into(), actual: "\"c\"".into() };
        assert_eq!(differs.describe(), r#"output line 2 differs: expected "b ", got "\"c\"""#);
    }
}
