//! What on a page could be a copied claim: lists, table columns, fenced blocks and sentences that
//! no directive binds. Values follow the fact rules: the first code span of an item, every code
//! span of a cell, else a bold lead term, else the visible text.

use std::collections::HashSet;

use super::prose::{SentenceCandidate, sentences_of_page};
use super::{Target, distinct};
use crate::attestations::directives::{Directive, parse_directive};
use crate::attestations::group::block_after_group;
use crate::attestations::placement::{Placement, place};
use crate::markdown::model::{Block, BlockKind, Comment, Page};
use crate::markdown::parse::{Positionless, parse_page_for};
use crate::text::is_word;

/// A list, or one column of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCandidate {
    pub target: Target,
    /// The first line of the list or table.
    pub line: usize,
    pub end: usize,
    pub items: Vec<String>,
    /// The column's header, as a `column=` directive names it.
    pub column: Option<String>,
    /// Active Markdown headings enclosing this candidate, outermost first.
    pub headings: Vec<String>,
}

/// A fenced block that is not an example.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCandidate {
    pub line: usize,
    pub text: String,
}

/// An untagged fenced code block that may compile against a configured surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleCandidate {
    pub line: usize,
    pub info: String,
    pub code: String,
}

/// Everything on one page `audit` may compare with a surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageCandidates {
    /// Lists, and the comparable columns of each table grouped together.
    pub sets: Vec<Vec<SetCandidate>>,
    pub blocks: Vec<BlockCandidate>,
    pub examples: Vec<ExampleCandidate>,
    pub sentences: Vec<SentenceCandidate>,
    /// The surface and expression of every fact on the page.
    pub attested: HashSet<(String, String)>,
}

/// Whether items can be compared as a set: two distinct non-empty items at least.
pub fn is_comparable(items: &[String]) -> bool {
    distinct(items.iter().map(String::as_str).filter(|item| !item.is_empty())).len() >= 2
}

/// Candidates of a page's text, using MDX preprocessing when `page` ends in `.mdx`.
pub fn candidates_of_page(page: &str, text: &str) -> Result<PageCandidates, Positionless> {
    let parsed = parse_page_for(page, text)?;
    let bound = bound_blocks(&parsed);
    let bound_lines: Vec<(usize, usize)> =
        bound.iter().filter_map(|at| parsed.flow.get(*at)).map(|b| (b.start, b.end)).collect();
    Ok(PageCandidates {
        sentences: sentences_of_page(page, text, &bound_lines, &opened_from_above(&parsed)),
        attested: attested(&parsed),
        ..blocks_of(&parsed, &bound)
    })
}

/// Candidates of a Markdown page's text.
#[cfg(test)]
pub fn candidates_of(text: &str) -> Result<PageCandidates, Positionless> {
    candidates_of_page("page.md", text)
}

/// The surface and expression of every fact directive on a page.
fn attested(page: &Page) -> HashSet<(String, String)> {
    page.comments
        .iter()
        .filter_map(|comment| match parse_directive(&comment.body, comment.line) {
            Some(Directive::Fact(fact)) => Some((fact.surface, fact.expression)),
            _ => None,
        })
        .collect()
}

/// The first lines of the paragraphs whose first sentence a value-binding directive on the line
/// above attests.
fn opened_from_above(page: &Page) -> Vec<usize> {
    page.comments
        .iter()
        .filter_map(|comment| {
            let directive = parse_directive(&comment.body, comment.line)?;
            if !directive.keyword().binds_values() {
                return None;
            }
            match place(page, comment, &directive) {
                Placement::Above(at) => page.flow.get(at.block).map(|b| b.start),
                _ => None,
            }
        })
        .collect()
}

/// The lists, tables and fenced blocks of a parsed page that `bound` leaves out.
fn blocks_of(page: &Page, bound: &HashSet<usize>) -> PageCandidates {
    let mut headings: Vec<(u8, String)> = Vec::new();
    let mut sets = Vec::new();
    for (at, block) in page.flow.iter().enumerate() {
        if let BlockKind::Heading { depth, text } = &block.kind {
            while headings.last().is_some_and(|(d, _)| d >= depth) {
                headings.pop();
            }
            headings.push((*depth, text.clone()));
            continue;
        }
        if bound.contains(&at) {
            continue;
        }
        let active: Vec<String> = headings.iter().map(|(_, text)| text.clone()).collect();
        if let Some(group) = block_sets(page, block, &active) {
            sets.push(group);
        }
    }
    let (blocks, examples) = unbound_code_blocks(page, bound);
    PageCandidates { sets, blocks, examples, ..PageCandidates::default() }
}

fn unbound_code_blocks(page: &Page, bound: &HashSet<usize>) -> (Vec<BlockCandidate>, Vec<ExampleCandidate>) {
    let mut blocks = Vec::new();
    let mut examples = Vec::new();
    for code in &page.code {
        if bound.contains(&code.block) {
            continue;
        }
        if let Some(block) = page.flow.get(code.block)
            && let BlockKind::Code { info, value } = &block.kind
            && !is_example(info)
        {
            blocks.push(BlockCandidate { line: block.start, text: value.clone() });
            examples.push(ExampleCandidate { line: block.start, info: info.clone(), code: value.clone() });
        }
    }
    (blocks, examples)
}

/// Whether `info` holds `check` or `compile` as a whole word (word characters are ASCII).
fn is_example(info: &str) -> bool {
    ["check", "compile"].iter().any(|tag| {
        info.match_indices(tag).any(|(at, _)| {
            let before = info[..at].chars().next_back();
            let after = info[at + tag.len()..].chars().next();
            !before.is_some_and(is_word) && !after.is_some_and(is_word)
        })
    })
}

// A list is one candidate, a table one candidate per column that could be compared.
fn block_sets(page: &Page, block: &Block, headings: &[String]) -> Option<Vec<SetCandidate>> {
    match &block.kind {
        BlockKind::List { items } => {
            let items: Vec<String> = items.iter().map(|i| i.value.clone()).collect();
            is_comparable(&items).then(|| {
                vec![SetCandidate {
                    target: Target::List,
                    line: block.start,
                    end: block.end,
                    items,
                    column: None,
                    headings: headings.to_vec(),
                }]
            })
        }
        BlockKind::Table(at) => {
            let table = page.tables.get(*at)?;
            let columns: Vec<SetCandidate> = table
                .header
                .iter()
                .enumerate()
                .filter(|(c, _)| !is_prose_column(table, *c))
                .map(|(c, column)| SetCandidate {
                    target: Target::Column,
                    line: block.start,
                    end: block.end,
                    items: table.column_values(c),
                    column: Some(column.clone()),
                    headings: headings.to_vec(),
                })
                .filter(|c| is_comparable(&c.items))
                .collect();
            (!columns.is_empty()).then_some(columns)
        }
        _ => None,
    }
}

/// Minimum words outside backticks for a cell to look like a prose description rather than a value.
const PROSE_WORDS_OUTSIDE_CODE: usize = 4;

/// Whether a table column consists mostly of prose description cells that merely mention inline
/// code spans (e.g. a `Description` column mentioning `` `devnet` `` and `` `mainnet` ``).
fn is_prose_column(table: &crate::markdown::model::Table, col: usize) -> bool {
    let mut non_empty = 0usize;
    let mut prose = 0usize;
    let mut has_code = false;
    for row in &table.rows {
        let Some(cell) = row.cells.get(col) else { continue };
        if cell.raw.trim().is_empty() {
            continue;
        }
        non_empty += 1;
        if cell.raw.contains('`') {
            has_code = true;
        }
        if prose_words_outside_code(&cell.raw) >= PROSE_WORDS_OUTSIDE_CODE {
            prose += 1;
        }
    }
    has_code && non_empty > 0 && prose * 2 >= non_empty
}

/// Count alphabetic words in `raw` outside backtick-delimited inline code spans.
fn prose_words_outside_code(raw: &str) -> usize {
    let mut outside = String::with_capacity(raw.len());
    let mut in_code = false;
    for ch in raw.chars() {
        if ch == '`' {
            in_code = !in_code;
            outside.push(' ');
        } else if !in_code {
            outside.push(ch);
        }
    }
    outside.split_whitespace().filter(|w| w.chars().any(char::is_alphabetic)).count()
}

// Conservative on purpose: a list, table or block right after any value-binding directive,
// where it takes effect, is treated as bound, and so is a table holding one in a cell.
// Blocks are named by their index in `page.flow`.
fn bound_blocks(page: &Page) -> HashSet<usize> {
    let table_block =
        |table: usize| page.flow.iter().position(|b| matches!(b.kind, BlockKind::Table(t) if t == table));
    let mut blocks = HashSet::new();
    for written in &page.comments {
        record_bound_comment(page, written, table_block, &mut blocks);
    }
    for open in &page.unclosed {
        if open.keyword.binds_values()
            && let Some(at) = open.table.and_then(table_block)
        {
            blocks.insert(at);
        }
    }
    blocks
}

/// Records the block a value-binding directive binds: the table it sits in, or the list,
/// table or fenced block right after it.
fn record_bound_comment(
    page: &Page,
    written: &Comment,
    table_block: impl Fn(usize) -> Option<usize>,
    blocks: &mut HashSet<usize>,
) {
    let Some(directive) = parse_directive(&written.body, written.line) else {
        return;
    };
    if !directive.keyword().binds_values() {
        return;
    }
    let placed = place(page, written, &directive);
    let comment = placed.comment().unwrap_or(written);
    if let Some(at) = comment.table.and_then(table_block) {
        blocks.insert(at);
    }
    if comment.ends_line
        && let Some((at, _)) = block_after_group(page, comment)
        && matches!(page.flow[at].kind, BlockKind::List { .. } | BlockKind::Table(_) | BlockKind::Code { .. })
    {
        blocks.insert(at);
    }
}

#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
