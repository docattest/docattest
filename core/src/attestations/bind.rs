//! Binding facts and cases tables to their targets.

use super::directives::FactDirective;
use super::field::bind_field;
use super::group::{GroupEnd, block_after_group};
use super::literals::literals;
use crate::api::attestation::{CasesRow, FactTarget, SetFrom};
use crate::markdown::model::{Block, BlockKind, Comment, Leaf, LeafKind, Page};
use crate::markdown::text::sentence_ends;
use crate::text::trim;

/// What a fact bound, and the rule that bound it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    pub target: FactTarget,
    pub rule: FactRule,
}

/// The rules [`bind_fact`] binds a fact by, in the order it tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactRule {
    /// `field=`: a value in the fenced block after it.
    Field,
    /// `column=`: a column of the table after it.
    Column,
    /// The list or fenced block that starts on the line after it.
    BlockBelow,
    /// The code span before it on its line.
    CodeSpan,
    /// The bare number or version before it on its line.
    Literal,
    /// The code spans of the sentence it ends, as a set.
    SentenceSpans,
    /// The one code span of the sentence it ends, on an earlier line of the sentence.
    SentenceSpan,
    /// The list or fenced block after the paragraph it ends, across blank lines.
    BlockAfter,
}

/// Bind a fact written on line `written` to its target, positionally and strictly, from
/// `comment`, the place it takes effect ([`super::placement`]). A directive that ends its line
/// binds the list, table (with `column=`) or fenced block that starts on the line after its
/// group ([`super::group`]); `column=` binds the next table, and `field=` a value in the next
/// fenced block, across blank lines. Otherwise it binds what is before it in its sentence (see
/// [`bind_inline`]); when nothing there is a target, a directive that ends its block binds the
/// list or fenced block right after that block, across blank lines. Anything else is an error.
pub fn bind_fact(
    fact: &FactDirective,
    written: usize,
    comment: &Comment,
    page: &Page,
) -> Result<Bound, String> {
    let (ordered, column) = (fact.ordered, fact.column.as_deref());
    let at = format!("fact at L{written}");
    let by = |rule: FactRule| move |target: FactTarget| Bound { target, rule };
    if let Some(field) = &fact.field {
        let next = next_past_comments(page, comment).filter(|_| comment.ends_line);
        return bind_field(&at, field, ordered, next).map(by(FactRule::Field));
    }
    if let Some(column) = column.filter(|c| !c.is_empty())
        && comment.ends_line
    {
        return bind_column(&at, column, next_past_comments(page, comment), page).map(by(FactRule::Column));
    }
    let after = block_after_group(page, comment);
    let adjacent = after
        .filter(|&(next, end)| comment.ends_line && page.flow[next].start == end.line + 1)
        .map(|(next, _)| &page.flow[next]);
    if let Some(bound) = bind_block(&at, adjacent, ordered) {
        return bound.map(by(FactRule::BlockBelow));
    }
    if let Some(bound) = bind_in_sentence(&at, comment, column, ordered) {
        return bound;
    }
    let following = after
        .filter(|&(next, end)| {
            ends_its_block(comment, page, end) && page.flow[next].previous == Some(end.block)
        })
        .map(|(next, _)| &page.flow[next]);
    let bound = bind_block(&at, following, ordered).unwrap_or_else(|| {
        let hint = "put it after a code span or number on the same line, at the end of a \
                    sentence with code spans, or at the end of a line or paragraph before a \
                    list, table or fenced block";
        Err(format!("{at} has no target: {hint}"))
    });
    bound.map(by(FactRule::BlockAfter))
}

/// What a fact binds in the sentence it ends: the candidate before it on its line, else the
/// code span of its sentence; none when neither is there.
fn bind_in_sentence(
    at: &str,
    comment: &Comment,
    column: Option<&str>,
    ordered: bool,
) -> Option<Result<Bound, String>> {
    let on_line = last_candidate(comment).map(|c| (c, true));
    let (candidate, on_line) = on_line.or_else(|| last_sentence_span(comment).map(|c| (c, false)))?;
    let scalar = inline_rule(&candidate, on_line);
    Some(bind_inline(at, candidate, comment, column, ordered).map(|target| {
        let rule = if matches!(target, FactTarget::Set { .. }) { FactRule::SentenceSpans } else { scalar };
        Bound { target, rule }
    }))
}

/// The rule an inline candidate binds by, as a scalar: from its own line, or from its sentence.
fn inline_rule(candidate: &Candidate, on_line: bool) -> FactRule {
    match (candidate, on_line) {
        (Candidate::Literal(_), _) => FactRule::Literal,
        (Candidate::Code(_), true) => FactRule::CodeSpan,
        (Candidate::Code(_), false) => FactRule::SentenceSpan,
    }
}

/// Nothing but its group follows the directive in the paragraph, heading or HTML block that
/// holds it.
fn ends_its_block(comment: &Comment, page: &Page, group: GroupEnd) -> bool {
    comment.ends_line && page.flow.get(comment.block).is_some_and(|b| b.end <= group.line)
}

/// The first block after the comment's own that is not an HTML block of comments, so that
/// the facts stacked above a table or a block, one per column or field, all reach it.
fn next_past_comments<'p>(page: &'p Page, comment: &Comment) -> Option<&'p Block> {
    (comment.block + 1..page.flow.len())
        .find(|&index| !page.is_only_comments(index))
        .map(|index| &page.flow[index])
}

/// `column=`: the column of the table that follows, across blank lines and the other
/// directives stacked above it.
fn bind_column(at: &str, column: &str, next: Option<&Block>, page: &Page) -> Result<FactTarget, String> {
    let Some(BlockKind::Table(t)) = next.map(|b| &b.kind) else {
        return Err(format!("{at} has column={column} but no table follows it"));
    };
    let table = &page.tables[*t];
    let Some(index) = table.header.iter().position(|h| h == column) else {
        let columns = table.header.join(", ");
        return Err(format!("{at}: the table has no column \"{column}\" (columns: {columns})"));
    };
    Ok(FactTarget::Set {
        items: table.column_values(index),
        from: SetFrom::Table { column: column.to_string() },
    })
}

/// The list or fenced block on the next line, if one is there.
fn bind_block(at: &str, adjacent: Option<&Block>, ordered: bool) -> Option<Result<FactTarget, String>> {
    match adjacent.map(|b| &b.kind) {
        Some(BlockKind::List { items }) => {
            let items = items.iter().map(|i| i.value.clone()).collect();
            Some(Ok(FactTarget::Set { items, from: SetFrom::List }))
        }
        Some(BlockKind::Table(_)) => {
            let hint = "use <!-- fact column=<name>: ... -->";
            Some(Err(format!("{at} is followed by a table but names no column; {hint}")))
        }
        Some(BlockKind::Code { value, .. }) => {
            if ordered {
                return Some(Err(format!(
                    "{at}: column= and ordered apply to lists and tables, not fenced blocks"
                )));
            }
            Some(Ok(FactTarget::Text(value.clone())))
        }
        _ => None,
    }
}

/// Bind `candidate`: the code span, number or version before the comment on its line, else the
/// code span of the sentence it ends (a sentence wrapped over lines); or, when that is a code
/// span that ends a sentence of two or more, the sentence's code spans as a set, which the
/// comparison reads as the last of them when the value is not a list.
fn bind_inline(
    at: &str,
    candidate: Candidate,
    comment: &Comment,
    column: Option<&str>,
    ordered: bool,
) -> Result<FactTarget, String> {
    let not_scalar = || format!("{at}: column= and ordered apply to lists and tables, not scalars");
    if column.is_some_and(|c| !c.is_empty()) {
        return Err(not_scalar());
    }
    let value = match candidate {
        Candidate::Code(value) => {
            let spans = sentence_spans(&comment.before);
            if spans.len() >= 2 {
                return Ok(FactTarget::Set { items: spans, from: SetFrom::Sentence });
            }
            value
        }
        Candidate::Literal(value) => value,
    };
    if ordered {
        return Err(not_scalar());
    }
    Ok(FactTarget::Scalar(value))
}

/// What a fact on a line of prose may compare: a code span, or a bare literal in the text.
enum Candidate {
    Code(String),
    Literal(String),
}

/// The last code span, bare number or version before the comment on the comment's own line.
fn last_candidate(comment: &Comment) -> Option<Candidate> {
    let mut found = None;
    for leaf in comment.before.iter().filter(|l| l.line == comment.line) {
        let candidate = match leaf.kind {
            LeafKind::Code => Some(Candidate::Code(trim(&leaf.value).to_string())),
            LeafKind::Text => literals(&leaf.value).pop().map(Candidate::Literal),
        };
        found = candidate.or(found);
    }
    found
}

/// The last code span of the sentence the comment ends, wherever its lines are.
fn last_sentence_span(comment: &Comment) -> Option<Candidate> {
    sentence_spans(&comment.before).pop().map(Candidate::Code)
}

/// The code spans of the sentence the leaves end, counted from the previous sentence end;
/// none when another sentence starts after the last code span.
fn sentence_spans(leaves: &[Leaf]) -> Vec<String> {
    let Some(last) = leaves.iter().rposition(|l| l.kind == LeafKind::Code) else {
        return Vec::new();
    };
    let trailing: Vec<&str> = leaves[last + 1..].iter().map(|l| l.value.as_str()).collect();
    if starts_a_sentence(&trailing.join("\n")) {
        return Vec::new();
    }
    let start = leaves[..last]
        .iter()
        .rposition(|l| l.kind == LeafKind::Text && ends_a_sentence(&l.value))
        .map_or(0, |k| k + 1);
    leaves[start..=last]
        .iter()
        .filter(|l| l.kind == LeafKind::Code)
        .map(|l| trim(&l.value).to_string())
        .collect()
}

fn ends_a_sentence(text: &str) -> bool {
    sentence_ends(text).next().is_some()
}

/// A sentence ends in `text` and more prose follows it.
fn starts_a_sentence(text: &str) -> bool {
    sentence_ends(text).any(|end| !trim(&text[end..]).is_empty())
}

/// Bind a cases directive to the table after it. A directive that ends its line can only
/// mean a table, so blank lines may separate them; anything else in between is an error.
pub fn bind_cases(comment: &Comment, page: &Page) -> Result<(usize, Vec<String>, Vec<CasesRow>), String> {
    let next = block_after_group(page, comment).map(|(index, _)| &page.flow[index]);
    let table_index = match next.map(|b| &b.kind) {
        Some(BlockKind::Table(t)) if comment.ends_line => *t,
        _ => {
            let hint = "end a line with the directive and start the table after it";
            return Err(format!("cases at L{} has no table: {hint}", comment.line));
        }
    };
    let table = &page.tables[table_index];
    let rows = table
        .rows
        .iter()
        .map(|row| {
            let mut values: Vec<(String, String)> = Vec::new();
            for (k, h) in table.header.iter().enumerate() {
                let raw = row.cells.get(k).map(|c| c.raw.clone()).unwrap_or_default();
                match values.iter_mut().find(|(key, _)| key == h) {
                    Some(existing) => existing.1 = raw,
                    None => values.push((h.clone(), raw)),
                }
            }
            CasesRow {
                line: row.line,
                name: row.cells.first().map(|c| c.raw.clone()).unwrap_or_default(),
                values,
            }
        })
        .collect();
    Ok((table_index, table.header.clone(), rows))
}

#[cfg(test)]
mod tests;
