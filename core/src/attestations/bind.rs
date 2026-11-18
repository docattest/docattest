//! Binding facts and cases tables to their targets.

use super::directives::FactDirective;
use super::field::bind_field;
use super::literals::literals;
use crate::api::attestation::{CasesRow, FactTarget, SetFrom};
use crate::markdown::model::{Block, BlockKind, Comment, Leaf, LeafKind, Page};
use crate::markdown::text::sentence_ends;
use crate::text::trim;

/// The first block that starts after the comment's last line.
fn next_block<'p>(page: &'p Page, comment: &Comment) -> Option<&'p Block> {
    page.flow.iter().find(|b| b.start > comment.end_line)
}

/// Bind a fact written on line `written` to its target, positionally and strictly, from
/// `comment`, the place it takes effect ([`super::placement`]). A directive that ends its line
/// binds the list, table (with `column=`) or fenced block that starts on the next line;
/// `column=` binds the next table, and `field=` a value in the next fenced block, across blank
/// lines. Otherwise it binds what is before it in its sentence (see [`bind_inline`]); when
/// nothing there is a target, a directive that ends its block binds the list or fenced block
/// right after that block, across blank lines. Anything else is an error.
pub fn bind_fact(
    fact: &FactDirective,
    written: usize,
    comment: &Comment,
    page: &Page,
) -> Result<FactTarget, String> {
    let (ordered, column) = (fact.ordered, fact.column.as_deref());
    let at = format!("fact at L{written}");
    if let Some(field) = &fact.field {
        let next = next_past_comments(page, comment).filter(|_| comment.ends_line);
        return bind_field(&at, field, ordered, next);
    }
    let next = next_block(page, comment);
    if let Some(column) = column.filter(|c| !c.is_empty())
        && comment.ends_line
    {
        return bind_column(&at, column, next_past_comments(page, comment), page);
    }
    let adjacent = next.filter(|b| comment.ends_line && b.start == comment.end_line + 1);
    if let Some(bound) = bind_block(&at, adjacent, ordered) {
        return bound;
    }
    if let Some(candidate) = last_candidate(comment).or_else(|| last_sentence_span(comment)) {
        return bind_inline(&at, candidate, comment, column, ordered);
    }
    let following = next.filter(|b| ends_its_block(comment, page) && b.previous == Some(comment.block));
    bind_block(&at, following, ordered).unwrap_or_else(|| {
        let hint = "put it after a code span or number on the same line, at the end of a \
                    sentence with code spans, or at the end of a line or paragraph before a \
                    list, table or fenced block";
        Err(format!("{at} has no target: {hint}"))
    })
}

/// Nothing follows the directive in the paragraph, heading or HTML block that holds it.
fn ends_its_block(comment: &Comment, page: &Page) -> bool {
    comment.ends_line && page.flow.get(comment.block).is_some_and(|b| b.end == comment.end_line)
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
    use crate::api::field::as_json_writes;
    use crate::attestations::field::parse_field_path;
    use crate::markdown::parse::parse_page;

    fn directive(ordered: bool, column: Option<&str>, field: Option<&str>) -> FactDirective {
        FactDirective {
            expression: "E".into(),
            surface: "default".into(),
            ordered,
            column: column.map(Into::into),
            field: field.map(|written| parse_field_path(written).expect("a path")),
        }
    }

    fn bound(text: &str, ordered: bool) -> Result<FactTarget, String> {
        let page = parse_page(text).unwrap_or_default();
        let last = page.comments.last().expect("a comment");
        bind_fact(&directive(ordered, None, None), last.line, last, &page)
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

    #[test]
    fn a_directive_ending_a_paragraph_with_no_target_binds_the_list_after_it() {
        let list = |items: &[&str]| {
            Ok(FactTarget::Set { items: items.iter().map(|s| s.to_string()).collect(), from: SetFrom::List })
        };
        assert_eq!(bound("Three keys: <!-- f -->\n\n- `a`\n- `b`", false), list(&["a", "b"]));
        assert_eq!(bound("<!-- f -->\n\n\n- `a`", false), list(&["a"]));
        assert_eq!(bound("Shape: <!-- f -->\n\n```ts\nx\n```", false), Ok(FactTarget::Text("x".into())));
        assert_eq!(bound("Keys are `a`. <!-- f -->\n\n- `b`", false), Ok(FactTarget::Scalar("a".into())));
        assert!(bound("Three keys: <!-- f --> more\n\n- `a`", false).is_err());
        assert!(bound("Three keys: <!-- f -->\n\nProse.\n\n- `a`", false).is_err());
        assert!(
            bound("Three keys: <!-- f -->\n\n| a |\n|---|\n| b |", false)
                .unwrap_err()
                .contains("names no column")
        );
    }

    #[test]
    fn column_facts_stacked_above_one_table_each_bind_it() {
        let column = |items: &[&str], name: &str| {
            Ok(FactTarget::Set {
                items: items.iter().map(|s| s.to_string()).collect(),
                from: SetFrom::Table { column: name.into() },
            })
        };
        let table = "| method | path |\n|---|---|\n| GET | `/a` |";
        for gap in ["\n", "\n\n"] {
            let page = parse_page(&format!("<!-- f -->{gap}<!-- g -->\n{table}")).unwrap_or_default();
            let [first, second] = &page.comments[..] else { panic!("two comments") };
            assert_eq!(
                bind_fact(&directive(false, Some("method"), None), first.line, first, &page),
                column(&["GET"], "method")
            );
            assert_eq!(
                bind_fact(&directive(false, Some("path"), None), second.line, second, &page),
                column(&["/a"], "path")
            );
        }
    }

    #[test]
    fn field_facts_stacked_above_one_block_each_bind_it() {
        let block = "```json\n{\"port\": 2222, \"hosts\": [\"a\", \"b\"]}\n```";
        for gap in ["\n", "\n\n"] {
            let page =
                parse_page(&format!("The job: <!-- f -->{gap}<!-- g -->\n{block}")).unwrap_or_default();
            let [first, second] = &page.comments[..] else { panic!("two comments") };
            let spelled = |target: Result<FactTarget, String>| target.map(|t| t.spelled(as_json_writes));
            let port = spelled(bind_fact(&directive(false, None, Some("port")), first.line, first, &page));
            assert_eq!(port, Ok(FactTarget::Scalar("2222".into())));
            let hosts = spelled(bind_fact(&directive(true, None, Some("hosts")), second.line, second, &page));
            let items = vec!["a".to_string(), "b".to_string()];
            assert_eq!(hosts, Ok(FactTarget::Set { items, from: SetFrom::Field { path: "hosts".into() } }));
        }
        let page = parse_page(&format!("Port `1` <!-- f --> here.\n{block}")).unwrap_or_default();
        let inline = bind_fact(&directive(false, None, Some("port")), 1, &page.comments[0], &page);
        assert_eq!(inline, Err("fact at L1 has field=port but no fenced block follows it".into()));
    }
}
