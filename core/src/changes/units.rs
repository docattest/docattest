//! Prose units: the places on a page a claim can live. A paragraph (at any depth, so list items
//! too), a table row, or the description of a doc comment. The last come from the language: the
//! core does not read source code.

use crate::adapter::DocProse;
use crate::api::attestation::Attestation;
use crate::markdown::model::{BlockKind, Page, TableRow};
use crate::markdown::text::{prose, sentence_ends};
use crate::text::trim;

/// A run of prose and the lines it spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// A claim found in a unit (by its index on the page); `offset` is the attestation's line
/// within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Located {
    pub unit: usize,
    pub offset: usize,
}

/// Paragraphs and table body rows of a parsed Markdown page, in order.
pub fn markdown_units(page: &Page) -> Vec<Unit> {
    let mut units = Vec::new();
    for block in &page.flow {
        match &block.kind {
            BlockKind::Paragraph { text } => {
                units.push(Unit { text: text.clone(), start: block.start, end: block.end });
            }
            BlockKind::Table(table) => {
                let rows = page.tables.get(*table).map_or(&[][..], |t| t.rows.as_slice());
                units.extend(rows.iter().map(|row| Unit {
                    text: row_text(row),
                    start: row.line,
                    end: row.line,
                }));
            }
            _ => {}
        }
    }
    units
}

fn row_text(row: &TableRow) -> String {
    row.cells.iter().map(|cell| prose(&cell.raw)).collect::<Vec<_>>().join(" | ")
}

/// The doc comments of a source file as units: what the language's `docProse` answered.
pub fn doc_comment_units(prose: Vec<DocProse>) -> Vec<Unit> {
    prose.into_iter().map(|p| Unit { text: p.text, start: p.start, end: p.end }).collect()
}

/// The prose an attestation defends: the unit holding its directive (the sentence carrying
/// the literal), else the unit a directive alone on the line above attests (`from_above`: the
/// line a directive is written on and the first line of that unit), else the unit its claim
/// names, nearest before it. Headings are never claims here: a heading names a topic, and
/// deleting one proof under it orphans nothing.
pub fn claim_unit(a: &Attestation, units: &[Unit], from_above: &[(usize, usize)]) -> Option<Located> {
    if let Some(inside) = units.iter().position(|u| u.start <= a.line && a.line <= u.end) {
        return Some(Located { unit: inside, offset: a.line - units[inside].start });
    }
    if let Some(&(_, start)) = from_above.iter().find(|&&(line, _)| line == a.line) {
        return units.iter().position(|u| u.start == start).map(|unit| Located { unit, offset: 0 });
    }
    if a.claim.is_empty() {
        return None;
    }
    let named = units.iter().rposition(|u| u.text == a.claim && u.start <= a.line)?;
    Some(Located { unit: named, offset: 0 })
}

/// The part of its unit an attestation defends: the sentence that starts with its claim (a
/// directive in running prose claims its sentence up to the directive), else the whole unit
/// (a block defends the paragraph before it).
pub fn claim_piece<'u>(a: &Attestation, unit: &'u Unit) -> &'u str {
    if a.claim.is_empty() {
        return &unit.text;
    }
    sentences(&unit.text).find(|s| s.starts_with(a.claim.as_str())).unwrap_or(&unit.text)
}

/// Every part of a unit a claim can be: each of its sentences, and the whole unit when it has
/// more than one.
pub fn pieces(unit: &Unit) -> Vec<&str> {
    let mut out: Vec<&str> = sentences(&unit.text).collect();
    if out.len() > 1 {
        out.push(&unit.text);
    }
    out
}

/// The sentences of a text, trimmed, in order; what follows the last sentence end is one too.
fn sentences(text: &str) -> impl Iterator<Item = &str> {
    let mut start = 0;
    sentence_ends(text).chain(std::iter::once(text.len())).filter_map(move |end| {
        let sentence = trim(&text[start..end]);
        start = end;
        (!sentence.is_empty()).then_some(sentence)
    })
}

/// The line of a located claim inside another unit with the same text.
pub fn line_in(unit: &Unit, offset: usize) -> usize {
    (unit.start + offset).min(unit.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::attestation::{Body, Test};
    use crate::markdown::parse::parse_page;

    fn unit(text: &str, start: usize, end: usize) -> Unit {
        Unit { text: text.to_string(), start, end }
    }

    fn attestation(claim: &str) -> Attestation {
        let body = Body::Test(Test { file: "t.ts".into(), title: String::new(), describes: Vec::new() });
        let (id, page) = ("x".to_string(), "p.md".to_string());
        Attestation { id, page, line: 1, claim: claim.to_string(), error: None, body }
    }

    #[test]
    fn a_claim_is_the_sentence_it_starts_else_the_whole_unit() {
        let both = unit("The user is `acme`. A call is tried `3` times.", 1, 1);
        assert_eq!(claim_piece(&attestation("A call is tried `3`"), &both), "A call is tried `3` times.");
        assert_eq!(claim_piece(&attestation("The user is `acme`."), &both), "The user is `acme`.");
        assert_eq!(claim_piece(&attestation("Elsewhere."), &both), both.text);
        assert_eq!(claim_piece(&attestation(""), &both), both.text);
        assert_eq!(pieces(&both), ["The user is `acme`.", "A call is tried `3` times.", both.text.as_str()]);
        assert_eq!(pieces(&unit("One `1` and no end", 1, 1)), ["One `1` and no end"]);
    }

    #[test]
    fn paragraphs_at_any_depth_and_table_rows_are_units() {
        let page = parse_page(
            "# Title\n\nFirst <!-- fact: N -->\nparagraph.\n\n- item one\n- item `two`\n\n| a | b |\n|---|---|\n| x <!-- c --> | `1` |\n| y | 2 |\n",
        )
        .unwrap();
        assert_eq!(
            markdown_units(&page),
            vec![
                unit("First paragraph.", 3, 4),
                unit("item one", 6, 6),
                unit("item `two`", 7, 7),
                unit("x | `1`", 11, 11),
                unit("y | 2", 12, 12),
            ]
        );
    }
}
