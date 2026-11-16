//! Binding facts and cases tables to their targets.

use super::literals::literals;
use crate::api::attestation::{CasesRow, FactTarget, SetFrom};
use crate::markdown::model::{Block, BlockKind, Comment, Leaf, LeafKind, Page};
use crate::markdown::text::sentence_ends;
use crate::text::trim;

/// The first block that starts after the comment's last line.
fn next_block<'p>(page: &'p Page, comment: &Comment) -> Option<&'p Block> {
    page.flow.iter().find(|b| b.start > comment.end_line)
}

/// Bind a fact to its target, positionally and strictly. A directive that ends its line binds
/// the list, table (with `column=`) or fenced block that starts on the next line; `column=`
/// binds the next table across blank lines. Otherwise it binds what is before it in its
/// sentence (see [`bind_inline`]). Anything else is an error.
pub fn bind_fact(
    ordered: bool,
    column: Option<&str>,
    comment: &Comment,
    page: &Page,
) -> Result<FactTarget, String> {
    let at = format!("fact at L{}", comment.line);
    let next = next_block(page, comment);
    if let Some(column) = column.filter(|c| !c.is_empty())
        && comment.ends_line
    {
        return bind_column(&at, column, next, page);
    }
    let adjacent = next.filter(|b| comment.ends_line && b.start == comment.end_line + 1);
    if let Some(bound) = bind_block(&at, adjacent, ordered) {
        return bound;
    }
    bind_inline(&at, comment, column, ordered)
}

/// `column=`: the column of the table that follows, across blank lines.
fn bind_column(at: &str, column: &str, next: Option<&Block>, page: &Page) -> Result<FactTarget, String> {
    let Some(BlockKind::Table(t)) = next.map(|b| &b.kind) else {
        return Err(format!("{at} has column={column} but no table follows it"));
    };
    let table = &page.tables[*t];
    let Some(index) = table.header.iter().position(|h| h == column) else {
        let columns = table.header.join(", ");
        return Err(format!("{at}: the table has no column \"{column}\" (columns: {columns})"));
    };
    let items =
        table.rows.iter().map(|r| r.cells.get(index).map(|c| c.value.clone()).unwrap_or_default()).collect();
    Ok(FactTarget::Set { items, from: SetFrom::Table { column: column.to_string() } })
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

/// The code span, number or version before the comment on its line, else the code span of
/// the sentence it ends (a sentence wrapped over lines); or, when that is a code span that
/// ends a sentence of two or more, the sentence's code spans as a set, which the comparison
/// reads as the last of them when the value is not a list.
fn bind_inline(
    at: &str,
    comment: &Comment,
    column: Option<&str>,
    ordered: bool,
) -> Result<FactTarget, String> {
    let Some(candidate) = last_candidate(comment).or_else(|| last_sentence_span(comment)) else {
        let hint = "put it after a code span or number on the same line, at the end of a \
                    sentence with code spans, or on the line before a list, table or fenced block";
        return Err(format!("{at} has no target: {hint}"));
    };
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
    let next = next_block(page, comment);
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
mod tests {
    use super::*;
    use crate::markdown::parse::parse_page;

    fn bound(text: &str, ordered: bool) -> Result<FactTarget, String> {
        let page = parse_page(text).unwrap_or_default();
        let last = page.comments.last().expect("a comment");
        bind_fact(ordered, None, last, &page)
    }

    fn sentence(items: &[&str]) -> Result<FactTarget, String> {
        Ok(FactTarget::Set { items: items.iter().map(|s| s.to_string()).collect(), from: SetFrom::Sentence })
    }

    #[test]
    fn a_sentence_of_code_spans_binds_them_all() {
        assert_eq!(bound("Keys are `a`, `b` and `c`. <!-- f -->", false), sentence(&["a", "b", "c"]));
        assert_eq!(bound("Keys are `a`,\n`b` and `c`. <!-- f -->", true), sentence(&["a", "b", "c"]));
        assert_eq!(bound("See `x`. Keys are `a` and `b`. <!-- f -->", false), sentence(&["a", "b"]));
        assert_eq!(bound("See `x`.\nKeys are `a` and `b`. <!-- f -->", false), sentence(&["a", "b"]));
        assert_eq!(bound("`x` <!-- c --> `a` and `b`. <!-- f -->", false), sentence(&["a", "b"]));
        assert_eq!(bound("| k |\n|---|\n| `a` and `b` <!-- f --> |", false), sentence(&["a", "b"]));
        assert_eq!(bound("- `x`. Keys: `a`, `b`. <!-- f -->", false), sentence(&["a", "b"]));
        assert_eq!(bound("Keys are `a` and `b`\nhere. <!-- f -->", false), sentence(&["a", "b"]));
    }

    #[test]
    fn a_single_value_stays_a_scalar() {
        assert_eq!(bound("Keys are `a`. <!-- f -->", false), Ok(FactTarget::Scalar("a".into())));
        assert_eq!(bound("`a` and `b` take 3 tries. <!-- f -->", false), Ok(FactTarget::Scalar("3".into())));
        assert_eq!(bound("`a` and `b`. Then more. <!-- f -->", false), Ok(FactTarget::Scalar("b".into())));
        assert_eq!(
            bound("Released as 1.2.3-rc.1. <!-- f -->", false),
            Ok(FactTarget::Scalar("1.2.3-rc.1".into()))
        );
        assert!(bound("Keys are `a`. <!-- f -->", true).is_err());
        assert_eq!(bound("The key is `a`\nhere. <!-- f -->", false), Ok(FactTarget::Scalar("a".into())));
        assert!(bound("See `x`. No value\nhere. <!-- f -->", false).is_err());
    }
}
