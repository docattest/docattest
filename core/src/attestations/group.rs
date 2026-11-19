//! Directives side by side. A directive that ends its line (nothing but other comments after
//! it, [`crate::markdown::text::ends_line_past_comments`]), the comments after it on that line
//! and the lines of only comments directly below, with no blank line between, are one group.
//! What follows a directive is what follows its group, so the order the directives of a group
//! are written in never changes what any of them binds.

use crate::markdown::model::{Comment, Page};

/// The first block that starts below `comment`'s group, as an index into [`Page::flow`], with
/// where the group ends.
pub fn block_after_group(page: &Page, comment: &Comment) -> Option<(usize, GroupEnd)> {
    let end = group_end(page, comment);
    page.flow.iter().position(|b| b.start > end.line).map(|index| (index, end))
}

/// Where a group of comments ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupEnd {
    /// Its last line.
    pub line: usize,
    /// Index into [`Page::flow`] of the block that holds its last comment.
    pub block: usize,
}

fn group_end(page: &Page, comment: &Comment) -> GroupEnd {
    let mut end = GroupEnd { line: comment.end_line, block: comment.block };
    if !comment.ends_line {
        return end;
    }
    end.line = run_end(page, comment);
    while let Some(below) = comments_directly_below(page, end) {
        end = GroupEnd { line: page.flow[below].end, block: below };
    }
    end
}

/// The last line of the comments after `comment` on its line: one of them may span lines,
/// and another start where it ends.
fn run_end(page: &Page, comment: &Comment) -> usize {
    let ends_later = |line: usize| {
        let starting_here = page.comments.iter().filter(|c| c.block == comment.block && c.line == line);
        starting_here.map(|c| c.end_line).max().filter(|&end| end > line)
    };
    let mut line = comment.end_line;
    while let Some(later) = ends_later(line) {
        line = later;
    }
    line
}

/// The block after the group's last one in its container, when it starts on the next line and
/// holds only comments on lines of their own.
fn comments_directly_below(page: &Page, end: GroupEnd) -> Option<usize> {
    let below = page.flow.iter().position(|b| b.previous == Some(end.block))?;
    (page.flow[below].start == end.line + 1 && page.is_alone_comments(below)).then_some(below)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse::parse_page;

    /// The last line of the first comment's group, and the line the block after it starts on.
    fn after(text: &str) -> Option<(usize, usize)> {
        let page = parse_page(text).unwrap_or_default();
        block_after_group(&page, &page.comments[0]).map(|(index, end)| (end.line, page.flow[index].start))
    }

    #[test]
    fn a_group_runs_over_the_comments_beside_and_directly_below_a_directive() {
        assert_eq!(after("Keys: <!-- f --> <!-- t -->\n- `a`"), Some((1, 2)));
        assert_eq!(after("Keys: <!-- f -->\n<!-- t -->\n<!-- u -->\n- `a`"), Some((3, 4)));
        assert_eq!(after("Keys: <!-- f --> <!-- t\nspans -->\n- `a`"), Some((2, 3)));
        assert_eq!(after("Keys: <!-- f -->\n<!-- t -->\n\n<!-- u -->\n- `a`"), Some((2, 4)));
        assert_eq!(after("Keys: <!-- f --> more\n<!-- t -->\n- `a`"), Some((1, 2)));
    }
}
