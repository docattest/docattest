//! The lines of a page that already belong to an attestation of a symbol.

use crate::api::attestation::Kind;
use crate::attestations::placement::attested_from_above;
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
/// own sentence), the paragraph below a directive alone on the line above it, and the list or
/// table a directive binds.
/// A code span there is part of the proof, not an unproven mention.
pub fn covered_lines(text: &str, proven: &[(&str, Vec<&Reference>)]) -> Covered {
    let page = parse_page(text).unwrap_or_default();
    let blocks: Vec<&Block> = page.blocks.iter().filter_map(|&k| page.flow.get(k)).collect();
    let from_above = attested_from_above(&page);
    let below = |r: &Reference| {
        let (_, block) = from_above.iter().find(|&&(line, _)| line == r.line)?;
        page.flow.get(*block).map(|b| (b.start, b.end))
    };
    let ranges = proven
        .iter()
        .map(|(symbol, refs)| {
            let ranges = refs.iter().flat_map(|r| ranges_of(&blocks, r).into_iter().chain(below(r)));
            ((*symbol).to_string(), ranges.collect())
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
