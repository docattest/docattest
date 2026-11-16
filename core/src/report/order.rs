//! Orders that make reports identical everywhere.

use std::cmp::Ordering;

use crate::text::compare_utf16;

/// Order by page, then line.
pub fn by_page_and_line(a: (&str, usize), b: (&str, usize)) -> Ordering {
    compare_utf16(a.0, b.0).then(a.1.cmp(&b.1))
}
