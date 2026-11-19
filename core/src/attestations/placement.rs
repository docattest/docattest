//! Where a directive takes effect: the one rule for which sentence a directive is about, used
//! by binding, claims, `audit`, `impact` and `check --base` alike.
//!
//! A directive after text on its line takes effect where it is written. One alone on a line
//! (nothing but directives on it) takes effect on the line directly below, like a
//! `// @ts-expect-error`: above a paragraph or heading, at the end of its first sentence.

use super::directives::{Directive, parse_directive};
use crate::markdown::model::{BlockKind, Comment, Page};

/// Where a directive takes effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement<'p> {
    /// Where it is written: after what it attests; alone on a line above a list, table or
    /// fenced block, or with no prose below it; or alone on the line right after a paragraph
    /// or heading, which it ends, with a blank line below.
    Written(&'p Comment),
    /// Alone on the line directly above a paragraph or heading: at the end of its first
    /// sentence, as this comment written there.
    Above(Comment),
    /// Alone on a line, with a blank line between it and the prose below and no prose
    /// directly above: it attests nothing.
    Detached,
}

impl Placement<'_> {
    /// The comment as it takes effect; none for a detached one.
    pub fn comment(&self) -> Option<&Comment> {
        match self {
            Placement::Written(comment) => Some(comment),
            Placement::Above(comment) => Some(comment),
            Placement::Detached => None,
        }
    }

    /// The placement by line, owning nothing of the page.
    pub fn placed(&self) -> Placed {
        match self {
            Placement::Written(_) => Placed::Written,
            Placement::Above(at) => Placed::Above { line: at.line },
            Placement::Detached => Placed::Detached,
        }
    }
}

/// Where a directive takes effect, by line, as [`Placement`] decides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    Written,
    /// Alone above a paragraph or heading: as if written at the end of its first sentence,
    /// which ends on `line`.
    Above {
        line: usize,
    },
    Detached,
}

/// Where `directive`, written as `comment`, takes effect. Only a directive that attests a
/// sentence moves: a citation, and a fact without `column=` or `field=`; the others bind the
/// table or block below them where they are written.
pub fn place<'p>(page: &'p Page, comment: &'p Comment, directive: &Directive) -> Placement<'p> {
    if !comment.alone || !attests_a_sentence(directive) {
        return Placement::Written(comment);
    }
    let Some(below) = beyond_comments(page, comment.block, Direction::Below) else {
        return Placement::Written(comment);
    };
    let first_sentence = page.first_sentence(below.block).filter(|_| is_prose(page, below.block));
    let Some(first_sentence) = first_sentence else {
        return Placement::Written(comment);
    };
    if below.adjacent {
        return Placement::Above(first_sentence.as_comment(&comment.body));
    }
    let above = beyond_comments(page, comment.block, Direction::Above);
    match above.filter(|a| a.adjacent && is_prose(page, a.block)) {
        Some(_) => Placement::Written(comment),
        None => Placement::Detached,
    }
}

/// Every directive on a page that is written alone above a paragraph or heading: the line it
/// is written on, and the index into [`Page::flow`] of the block whose first sentence it attests.
pub fn attested_from_above(page: &Page) -> Vec<(usize, usize)> {
    let above = |comment: &Comment| match place(page, comment, &parse_directive(&comment.body, comment.line)?)
    {
        Placement::Above(at) => Some((comment.line, at.block)),
        _ => None,
    };
    page.comments.iter().filter_map(above).collect()
}

/// The error a detached directive is: what it should have been written next to.
pub fn detached_error(kind: &str, line: usize) -> String {
    format!(
        "{kind} at L{line} must sit directly above the sentence it attests, with no blank \
         line between, or at the end of that sentence"
    )
}

fn attests_a_sentence(directive: &Directive) -> bool {
    match directive {
        Directive::Test(_) => true,
        Directive::Fact(fact) => fact.column.is_none() && fact.field.is_none(),
        Directive::Cases { .. } | Directive::Alias(_) | Directive::Invalid { .. } => false,
    }
}

fn is_prose(page: &Page, block: usize) -> bool {
    page.flow
        .get(block)
        .is_some_and(|b| matches!(b.kind, BlockKind::Paragraph { .. } | BlockKind::Heading { .. }))
}

#[derive(Clone, Copy)]
enum Direction {
    Above,
    Below,
}

/// A block next to a directive's lines, in the same container.
struct Neighbour {
    block: usize,
    /// No blank line parts it from the directive.
    adjacent: bool,
}

/// The first block above or below `block` in its container that is not lines of comments,
/// stepping over the directives stacked with it.
fn beyond_comments(page: &Page, block: usize, direction: Direction) -> Option<Neighbour> {
    let (mut current, mut adjacent) = (block, true);
    loop {
        let next = match direction {
            Direction::Above => page.flow.get(current)?.previous?,
            Direction::Below => page.flow.iter().position(|b| b.previous == Some(current))?,
        };
        let (upper, lower) = match direction {
            Direction::Above => (next, current),
            Direction::Below => (current, next),
        };
        adjacent &= page.flow[upper].end + 1 == page.flow[lower].start;
        if !page.is_alone_comments(next) {
            return Some(Neighbour { block: next, adjacent });
        }
        current = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse::parse_page;

    /// Where each directive on a page takes effect: the line it acts on, or None when detached.
    fn placed(text: &str) -> Vec<Option<usize>> {
        let page = parse_page(text).unwrap_or_default();
        let directives = page.comments.iter().filter_map(|c| Some((c, parse_directive(&c.body, c.line)?)));
        directives.map(|(c, d)| place(&page, c, &d).comment().map(|at| at.line)).collect()
    }

    #[test]
    fn a_directive_alone_on_a_line_acts_on_the_sentence_directly_below() {
        assert_eq!(placed("<!-- fact: A -->\nPrices are `EUR`. More.\n"), [Some(2)]);
        assert_eq!(
            placed("<!-- fact: A --> <!-- test: t > \"x\" -->\nA wrapped\nsentence.\n"),
            [Some(3), Some(3)]
        );
        assert_eq!(placed("<!-- fact: A -->\n<!-- fact: B -->\n## Heading\n"), [Some(3), Some(3)]);
        assert_eq!(placed("- <!-- fact: A -->\n  An item.\n"), [Some(2)]);
    }

    #[test]
    fn a_directive_after_text_or_above_a_block_acts_where_it_is_written() {
        assert_eq!(placed("Text. <!-- fact: A -->\nMore.\n"), [Some(1)]);
        assert_eq!(placed("<!-- fact: A -->\n\n- `a`\n"), [Some(1)]);
        assert_eq!(placed("<!-- fact column=k: A -->\nProse.\n"), [Some(1)]);
        assert_eq!(placed("<!-- cases: run -->\nProse.\n"), [Some(1)]);
        assert_eq!(placed("A paragraph.\n<!-- test: t > \"x\" -->\n\nNext.\n"), [Some(2)]);
        assert_eq!(placed("- Item.\n  <!-- test: t > \"x\" -->\n- Next item.\n"), [Some(2)]);
    }

    #[test]
    fn a_directive_alone_with_a_blank_line_before_the_prose_below_is_detached() {
        assert_eq!(placed("<!-- fact: A -->\n\nPrices are `EUR`.\n"), [None]);
        assert_eq!(placed("Text.\n\n<!-- test: t > \"x\" -->\n\n## Next\n"), [None]);
        assert_eq!(placed("<!-- fact: A -->\n\n<!-- fact: B -->\nProse.\n"), [None, Some(4)]);
    }
}
