//! Where a token of a parsed file is in the file's text, in bytes.
//!
//! A span knows its line and its column, and the column counts characters, while an edit
//! is placed in bytes: a character before a token on its line may take more than one.

use proc_macro2::{LineColumn, Span};
use std::ops::Range;

/// The text of one file, with where each of its lines begins.
pub(super) struct Offsets<'t> {
    text: &'t str,
    line_starts: Vec<usize>,
}

impl<'t> Offsets<'t> {
    pub(super) fn new(text: &'t str) -> Self {
        // The parser does not see a byte order mark, so its first line begins after one.
        let first = if text.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
        let after_newlines = text.match_indices('\n').map(|(at, _)| at + 1);
        Offsets { text, line_starts: std::iter::once(first).chain(after_newlines).collect() }
    }

    /// The bytes a span covers.
    pub(super) fn range(&self, span: Span) -> Range<usize> {
        self.byte(span.start())..self.byte(span.end())
    }

    /// The text a span covers.
    pub(super) fn text(&self, span: Span) -> &'t str {
        &self.text[self.range(span)]
    }

    fn byte(&self, at: LineColumn) -> usize {
        let line_start = self.line_starts[at.line - 1];
        let line = &self.text[line_start..];
        line_start + line.char_indices().nth(at.column).map_or(line.len(), |(offset, _)| offset)
    }
}
