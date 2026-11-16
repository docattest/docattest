//! Prose units: the places on a page a claim can live. A paragraph (at any depth, so list items
//! too), a table row, or the description of a doc comment. The last come from the language: the
//! core does not read source code.

use crate::adapter::DocProse;
use crate::api::attestation::Attestation;
use crate::markdown::model::{BlockKind, Page, TableRow};
use crate::markdown::text::prose;

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
/// the literal), else the unit its claim names, nearest before it. Headings are never claims
/// here: a heading names a topic, and deleting one proof under it orphans nothing.
pub fn claim_unit(a: &Attestation, units: &[Unit]) -> Option<Located> {
    if let Some(inside) = units.iter().position(|u| u.start <= a.line && a.line <= u.end) {
        return Some(Located { unit: inside, offset: a.line - units[inside].start });
    }
    if a.claim.is_empty() {
        return None;
    }
    let named = units.iter().rposition(|u| u.text == a.claim && u.start <= a.line)?;
    Some(Located { unit: named, offset: 0 })
}

/// The line of a located claim inside another unit with the same text.
pub fn line_in(unit: &Unit, offset: usize) -> usize {
    (unit.start + offset).min(unit.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse::parse_page;

    fn unit(text: &str, start: usize, end: usize) -> Unit {
        Unit { text: text.to_string(), start, end }
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
