//! docattest's own tags in an info string, and the words that are likely misspellings of
//! them: a renderer's word on a block that is documentation is left alone, a misspelled tag
//! is not, so a typo can never turn a proof into prose.

/// The tags docattest reads after a block's language.
pub(super) const TAGS: [&str; 3] = ["check", "compile", "network"];

/// A word on an info string that is no known tag, as its error reports it.
pub(super) struct UnknownWord<'a> {
    pub(super) word: &'a str,
    /// The tag the word is likely a misspelling of.
    pub(super) meant: Option<&'static str>,
}

/// The one of a block's `unknown` words its error reports: the first likely misspelling of
/// a tag, since that one may be turning a proof into prose, or else the first word.
pub(super) fn unknown_to_report<'a>(unknown: impl IntoIterator<Item = &'a str>) -> Option<UnknownWord<'a>> {
    let words: Vec<&str> = unknown.into_iter().collect();
    let misspelled = words
        .iter()
        .find_map(|&word| misspelling_of(word).map(|meant| UnknownWord { word, meant: Some(meant) }));
    misspelled.or_else(|| words.first().map(|&word| UnknownWord { word, meant: None }))
}

/// The tag `word` is likely a misspelling of: one within a third of the tag's length in
/// edits (at least one), ignoring case, so `chekc`, `Check` and `complie` are and
/// `twoslash` is not. A tag itself is no misspelling.
fn misspelling_of(word: &str) -> Option<&'static str> {
    if TAGS.contains(&word) {
        return None;
    }
    let lower: Vec<char> = word.to_lowercase().chars().collect();
    TAGS.iter().copied().find(|tag| {
        let tag_chars: Vec<char> = tag.chars().collect();
        edits(&lower, &tag_chars) <= (tag_chars.len() / 3).max(1)
    })
}

/// The optimal string alignment distance: insertions, deletions, substitutions and swaps of
/// two adjacent letters, each one edit.
fn edits(a: &[char], b: &[char]) -> usize {
    let mut rows = vec![(0..=b.len()).collect::<Vec<usize>>()];
    for i in 1..=a.len() {
        let mut row = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let substitution = rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]);
            row[j] = substitution.min(rows[i - 1][j] + 1).min(row[j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                row[j] = row[j].min(rows[i - 2][j - 2] + 1);
            }
        }
        rows.push(row);
    }
    rows[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_misses_of_a_tag_are_misspellings() {
        for (word, tag) in [
            ("chekc", "check"),
            ("chek", "check"),
            ("checks", "check"),
            ("Check", "check"),
            ("complie", "compile"),
            ("compil", "compile"),
            ("netwrok", "network"),
            ("netwk", "network"),
        ] {
            assert_eq!(misspelling_of(word), Some(tag), "{word}");
        }
    }

    #[test]
    fn renderer_words_and_tags_are_not() {
        for word in ["twoslash", "title", "linenums", "no_run", "ignore", "copy", "chk", "check", "network"] {
            assert_eq!(misspelling_of(word), None, "{word}");
        }
    }
}
