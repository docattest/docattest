//! Where a fenced block on a page first parts from the text a fact observed.

use crate::text::{lines, words};

/// The first place a page's block and an observed text differ, whitespace ignored.
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

/// A word and the line of its text (1-based) it is on.
struct Word<'a> {
    text: &'a str,
    line: usize,
}

/// Where `page` and `observed` first differ word by word, so that wrapping a block's lines
/// differently is never a difference; None when they have the same words.
pub fn first_difference(page: &str, observed: &str) -> Option<Difference> {
    let (page, observed) = (words_by_line(page), words_by_line(observed));
    let at = (0..page.len().max(observed.len()))
        .find(|&k| page.get(k).map(|w| w.text) != observed.get(k).map(|w| w.text))?;
    let line = page.get(at).map_or_else(|| page.last().map_or(1, |w| w.line + 1), |w| w.line);
    // Every word before `at` is the same on both sides, so the line's first word starts
    // the same stretch of the observed text.
    let from = page.iter().position(|w| w.line == line).unwrap_or(at);
    let expected = page.get(at).map(|_| joined(page.iter().filter(|w| w.line == line)));
    let to = match observed.get(at) {
        Some(word) => observed.iter().rposition(|w| w.line == word.line).map_or(at, |k| k + 1),
        None => observed.len(),
    };
    let actual = (from < to).then(|| joined(observed[from..to].iter()));
    Some(Difference { line, expected, actual })
}

fn words_by_line(text: &str) -> Vec<Word<'_>> {
    lines(text)
        .into_iter()
        .enumerate()
        .flat_map(|(k, line)| words(line).into_iter().map(move |text| Word { text, line: k + 1 }))
        .collect()
}

fn joined<'a>(words: impl Iterator<Item = &'a Word<'a>>) -> String {
    words.map(|w| w.text).collect::<Vec<_>>().join(" ")
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
