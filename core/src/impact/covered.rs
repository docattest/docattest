//! The lines of a page that already belong to an attestation of a symbol.

use crate::api::attestation::Kind;
use crate::markdown::model::{Block, BlockKind};
use crate::markdown::parse::parse_page;
use crate::project::check::Reference;

/// Lines of a page that belong to an attestation of a symbol, by symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Covered {
    ranges: Vec<(String, Vec<(usize, usize)>)>,
}

impl Covered {
    /// Whether `line` belongs to an attestation of `symbol`.
    pub fn covers(&self, symbol: &str, line: usize) -> bool {
        self.ranges
            .iter()
            .filter(|(s, _)| s == symbol)
            .any(|(_, ranges)| ranges.iter().any(|&(start, end)| line >= start && line <= end))
    }
}

/// The lines each symbol's attestations occupy on a page: the attestation's own block, the
/// paragraph before it when that paragraph is its claim (not when the attestation claims its
/// own sentence), and the list or table a directive binds.
/// A code span there is part of the proof, not an unproven mention.
pub fn covered_lines(text: &str, proven: &[(&str, Vec<&Reference>)]) -> Covered {
    let page = parse_page(text).unwrap_or_default();
    let blocks: Vec<&Block> = page.blocks.iter().filter_map(|&k| page.flow.get(k)).collect();
    let ranges = proven
        .iter()
        .map(|(symbol, refs)| {
            ((*symbol).to_string(), refs.iter().flat_map(|r| ranges_of(&blocks, r)).collect())
        })
        .collect();
    Covered { ranges }
}

fn ranges_of(blocks: &[&Block], reference: &Reference) -> Vec<(usize, usize)> {
    let line = reference.line;
    let Some(at) = blocks.iter().position(|b| b.start <= line && line <= b.end) else {
        return vec![(line, line)];
    };
    let own = blocks[at];
    let mut out = vec![(own.start, own.end)];
    if let Some(before) = at.checked_sub(1).and_then(|k| blocks.get(k))
        && matches!(&before.kind, BlockKind::Paragraph { text } if *text == reference.claim)
    {
        out.push((before.start, before.end));
    }
    if let Some(after) = blocks.get(at + 1)
        && matches!(after.kind, BlockKind::List { .. } | BlockKind::Table(_))
        && (reference.kind == Kind::Cases || after.start == own.end + 1)
    {
        out.push((after.start, after.end));
    }
    out
}
