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
}

/// A fenced block that is not an example.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCandidate {
    pub line: usize,
    pub text: String,
}

/// Everything on one page `audit` may compare with a surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageCandidates {
    /// Lists, and the comparable columns of each table grouped together.
    pub sets: Vec<Vec<SetCandidate>>,
    pub blocks: Vec<BlockCandidate>,
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
    let sets = page
        .flow
        .iter()
        .enumerate()
        .filter(|(at, _)| !bound.contains(at))
        .filter_map(|(_, block)| block_sets(page, block))
        .collect();
    let blocks = page
        .code
        .iter()
        .filter(|code| !bound.contains(&code.block))
        .filter_map(|code| page.flow.get(code.block))
        .filter_map(|block| match &block.kind {
            BlockKind::Code { info, value } if !is_example(info) => {
                Some(BlockCandidate { line: block.start, text: value.clone() })
            }
            _ => None,
        })
        .collect();
    PageCandidates { sets, blocks, ..PageCandidates::default() }
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
fn block_sets(page: &Page, block: &Block) -> Option<Vec<SetCandidate>> {
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
                }]
            })
        }
        BlockKind::Table(at) => {
            let table = page.tables.get(*at)?;
            let columns: Vec<SetCandidate> = table
                .header
                .iter()
                .enumerate()
                .map(|(c, column)| SetCandidate {
                    target: Target::Column,
                    line: block.start,
                    end: block.end,
                    items: table.column_values(c),
                    column: Some(column.clone()),
                })
                .filter(|c| is_comparable(&c.items))
                .collect();
            (!columns.is_empty()).then_some(columns)
        }
        _ => None,
    }
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
mod tests {
    use super::*;

    fn found(lines: &[&str]) -> PageCandidates {
        candidates_of(&(lines.join("\n") + "\n")).unwrap()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn column(line: usize, end: usize, items: &[&str], name: &str) -> SetCandidate {
        SetCandidate { target: Target::Column, line, end, items: strings(items), column: Some(name.into()) }
    }

    #[test]
    fn takes_lists_and_table_columns_with_fact_item_values() {
        let found = found(&[
            "Keys:",
            "",
            "- `A` the first",
            "- **B**: a bold lead term",
            "- plain text",
            "",
            "| Method | Path |",
            "|---|---|",
            "| GET | `/a` and `/b` |",
            "| POST | `/c` |",
        ]);
        assert_eq!(
            found.sets,
            vec![
                vec![SetCandidate {
                    target: Target::List,
                    line: 3,
                    end: 5,
                    items: strings(&["A", "B", "plain text"]),
                    column: None
                }],
                vec![column(7, 10, &["GET", "POST"], "Method"), column(7, 10, &["/a", "/b", "/c"], "Path")],
            ]
        );
    }

    #[test]
    fn leaves_out_lists_and_columns_with_fewer_than_two_distinct_items() {
        let found = found(&[
            "- `A`",
            "- `A`",
            "",
            "| Name | Kind |",
            "|---|---|",
            "| `x` | same |",
            "| `y` | same |",
        ]);
        assert_eq!(found.sets, vec![vec![column(4, 7, &["x", "y"], "Name")]]);
    }

    #[test]
    fn skips_whatever_a_directive_binds() {
        let found = found(&[
            "Keys: <!-- fact: KEYS -->",
            "- `A`",
            "",
            "<!-- fact column=Path: P -->",
            "",
            "| Path |",
            "|---|",
            "| `/a` |",
            "",
            "<!-- fact: SHAPE -->",
            "```ts",
            "x",
            "```",
            "",
            "Port `8080`. <!-- fact: PORT -->",
            "",
            "| limit | value |",
            "|---|---|",
            "| tries | `3` <!-- fact: N --> |",
            "",
            "Also `9090`.",
        ]);
        assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
        assert_eq!(found.blocks, Vec::new());
    }

    #[test]
    fn leaves_out_the_sentence_a_directive_on_the_line_above_attests() {
        let found = found(&[
            "<!-- fact: PORT -->",
            "Port `8080`. Also `9090`.",
            "",
            "<!-- fact: KEYS -->",
            "Keys:",
            "- `a`",
            "- `b`",
        ]);
        let spans: Vec<&[String]> = found.sentences.iter().map(|s| s.spans.as_slice()).collect();
        assert_eq!(spans, [["9090".to_string()]]);
        assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
    }

    #[test]
    fn treats_a_malformed_directive_as_binding_too() {
        let found = found(&["<!-- fact KEYS -->", "- `A`", "- `B`"]);
        assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
    }

    #[test]
    fn a_test_citation_binds_no_list_and_no_sentence_below_it() {
        let found = found(&[
            "<!-- test: a.test.ts > \"keys\" -->",
            "Reads `/etc/app.conf`.",
            "",
            "<!-- test: a.test.ts > \"keys\" -->",
            "- `A`",
            "- `B`",
        ]);
        let spans: Vec<&[String]> = found.sentences.iter().map(|s| s.spans.as_slice()).collect();
        assert_eq!(spans[0], ["/etc/app.conf".to_string()]);
        assert_eq!(found.sets.len(), 1);
    }

    #[test]
    fn keeps_documentation_blocks_and_leaves_out_examples() {
        let found = found(&["```yaml", "a: 1", "```", "", "```ts check", "x", "```"]);
        assert_eq!(found.blocks, vec![BlockCandidate { line: 1, text: "a: 1".into() }]);
    }

    #[test]
    fn looks_inside_nothing_fenced() {
        let found = found(&["```md", "- `eu`", "- `us`", "Port `8080`.", "```"]);
        assert_eq!(found.sets, Vec::<Vec<SetCandidate>>::new());
    }

    #[test]
    fn example_tags_need_word_boundaries() {
        assert!(is_example("ts check"));
        assert!(is_example("ts title=x,compile"));
        assert!(is_example("check-me"));
        assert!(!is_example("ts checked"));
        assert!(!is_example("ts precompile"));
        assert!(!is_example("ts check_"));
        assert!(is_example("ts chécheck")); // `é` is not a word character: `\w` is ASCII
    }
}
