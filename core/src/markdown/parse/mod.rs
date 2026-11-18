//! Markdown (CommonMark, GFM, YAML frontmatter) to the page model.
//!
//! A page is read as `mdast-util-from-markdown` reads it, with micromark's GFM and YAML
//! frontmatter extensions: the lines and the slices of the page in every report are that
//! tree's, as the recordings hold. markdown-rs builds the same mdast tree with the same
//! positions, apart from the differences fixed up below. Offsets here are bytes where
//! micromark's are UTF-16 units, but each is only used to slice its own string, so the
//! slices agree.

use markdown::mdast::{Node, Table as MdTable, TableCell as MdCell};
use markdown::{Constructs, ParseOptions};

use super::autolink::transform_fires;
use super::comments::{CommentContext, flow_comments, phrasing_comments};
use super::frontmatter::freezes;
use super::model::{Block, BlockKind, CodeRef, ListItem, Page, Table, TableCell, TableRow};
use super::text::{cell_values, content_range, item_value, prose, slice};
use crate::api::errors::Error;
use crate::text::trim;

/// The tree holds nodes without positions (see [`super::autolink`]): no line can be given
/// for them, so the page fails with an internal error.
/// Also stands for a panic inside markdown-rs, which is reported the same way: as an
/// internal error at that page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Positionless {
    Text,
    Panicked,
}

impl Positionless {
    /// The internal error this is on `page`: whoever reports it needs to know which page.
    pub fn on(&self, page: &str) -> Error {
        Error::internal(format!("{page}: {self}"))
    }
}

impl std::fmt::Display for Positionless {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Positionless::Text => f.write_str("markdown node \"text\" has no position"),
            Positionless::Panicked => f.write_str("the Markdown parser failed on a page"),
        }
    }
}

/// Parse a Markdown page into the page model.
pub fn parse_page(text: &str) -> Result<Page, Positionless> {
    let source = normalise_newlines(text);
    // markdown-rs has panicked on input micromark accepts (see `options`); should it again,
    // the page fails as an internal error instead of aborting the process.
    let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        markdown::to_mdast(&source, &options(&source))
    }));
    let Ok(mut tree) = parsed else {
        return Err(Positionless::Panicked);
    };
    if let Ok(root) = tree.as_mut() {
        flatten_nested_lists(root);
        let newlines: Vec<usize> = source.match_indices('\n').map(|(k, _)| k).collect();
        relines(root, &newlines);
        if source.starts_with('\u{FEFF}') {
            shift_after_bom(root, &source);
        }
    }
    let mut builder = PageBuilder { source: &source, page: Page::default(), positionless: false };
    // Without MDX, parsing never fails; an empty page is the safe answer if it ever did.
    if let Ok(Node::Root(root)) = tree {
        let mut previous = None;
        for node in &root.children {
            if let Node::Yaml(yaml) = node {
                builder.page.frozen = freezes(&yaml.value);
            } else {
                let index = builder.visit(node, None, previous);
                builder.page.blocks.push(index);
                previous = Some(index);
            }
        }
    }
    if builder.positionless {
        return Err(Positionless::Text);
    }
    Ok(builder.page)
}

/// The mdast tree of a page as `mdast-util-from-markdown` builds it (the fix-ups of
/// [`parse_page`] applied), with the source its offsets index. None when the parser fails.
pub fn parse_tree(text: &str) -> Option<(String, Node)> {
    let source = normalise_newlines(text);
    let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        markdown::to_mdast(&source, &options(&source))
    }));
    let mut tree = parsed.ok()?.ok()?;
    flatten_nested_lists(&mut tree);
    let newlines: Vec<usize> = source.match_indices('\n').map(|(k, _)| k).collect();
    relines(&mut tree, &newlines);
    if source.starts_with('\u{FEFF}') {
        shift_after_bom(&mut tree, &source);
    }
    Some((source, tree))
}

/// markdown-rs nests the next items of a list in a list of their own, inside the list,
/// after an item whose fenced code block is never closed; micromark keeps them as items of
/// the one list. Put them back.
fn flatten_nested_lists(node: &mut Node) {
    if let Node::List(list) = node
        && list.children.iter().any(|c| matches!(c, Node::List(_)))
    {
        let children = std::mem::take(&mut list.children);
        for child in children {
            match child {
                Node::List(inner) => list.children.extend(inner.children),
                other => list.children.push(other),
            }
        }
    }
    if let Some(children) = node.children_mut() {
        children.iter_mut().for_each(flatten_nested_lists);
    }
}

/// Lines from offsets. markdown-rs's offsets are right, but its line numbers can lag behind
/// them in long paragraphs with non-ASCII text, so every line is recomputed from its
/// offset: one plus the line feeds before it.
fn relines(node: &mut Node, newlines: &[usize]) {
    if let Some(p) = node.position_mut() {
        p.start.line = 1 + newlines.partition_point(|&k| k < p.start.offset);
        p.end.line = 1 + newlines.partition_point(|&k| k < p.end.offset);
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            relines(child, newlines);
        }
    }
}

/// micromark skips a leading byte order mark without counting it, so every offset is one
/// UTF-16 unit short, and slices of the page (which keeps the mark) start and end a
/// character early, as the recordings hold. markdown-rs counts the mark; shift back to
/// match.
fn shift_after_bom(node: &mut Node, source: &str) {
    let back = |offset: usize| (0..offset).rev().find(|&k| source.is_char_boundary(k)).unwrap_or(0);
    if let Some(p) = node.position_mut() {
        p.start.offset = back(p.start.offset);
        p.end.offset = back(p.end.offset);
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            shift_after_bom(child, source);
        }
    }
}

/// Every `\r\n` and lone `\r` as `\n`.
fn normalise_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

// Only YAML frontmatter is read; markdown-rs's frontmatter construct also
// takes TOML (`+++`), and panics on an opening fence that never closes when a blank line
// and a container follow (`---\n\n> a`). So the construct is on only for a page that
// opens with a YAML fence and closes it, where it succeeds as micromark's does.
fn options(source: &str) -> ParseOptions {
    let mut constructs = Constructs::gfm();
    constructs.frontmatter = has_frontmatter(source);
    ParseOptions { constructs, ..ParseOptions::gfm() }
}

/// micromark's YAML frontmatter: a first line `---` (then only spaces or tabs, then a line
/// ending) and a later line of the same form (ending at a line ending or the end).
fn has_frontmatter(source: &str) -> bool {
    let source = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    let fence =
        |line: &str| line.strip_prefix("---").is_some_and(|rest| rest.chars().all(|c| c == ' ' || c == '\t'));
    let mut lines = source.split('\n');
    let opens = lines.next().is_some_and(fence) && source.contains('\n');
    opens && lines.any(fence)
}

struct PageBuilder<'a> {
    source: &'a str,
    page: Page,
    positionless: bool,
}

impl PageBuilder<'_> {
    // Pre-order, so a list comes before its items and binding finds the list first.
    // `previous` is the block before this one in its container (see `Block::previous`).
    fn visit(&mut self, node: &Node, parent: Option<&Node>, previous: Option<usize>) -> usize {
        let block = self.block_of(node, parent, previous);
        let index = self.page.flow.len();
        let table = match block.kind {
            BlockKind::Table(t) => Some(t),
            _ => None,
        };
        let is_code = matches!(block.kind, BlockKind::Code { .. });
        self.page.flow.push(block);
        if is_code {
            self.page.code.push(CodeRef { block: index });
        }
        self.visit_content(node, index, table);
        index
    }

    /// The comments in a block's own text, then the blocks inside it.
    fn visit_content(&mut self, node: &Node, index: usize, table: Option<usize>) {
        match node {
            Node::Html(html) => flow_comments(html, &mut self.context(index, None)),
            Node::Paragraph(p) => self.phrasing(&p.children, index, None),
            Node::Heading(h) => self.phrasing(&h.children, index, None),
            Node::Table(t) => {
                for row in &t.children {
                    for cell in row.children().map(Vec::as_slice).unwrap_or_default() {
                        let children = cell.children().map(Vec::as_slice).unwrap_or_default();
                        self.phrasing(children, index, table);
                    }
                }
            }
            _ => self.visit_children(node, index),
        }
    }

    /// The blocks inside a container. Its first block follows what the container follows;
    /// a list's items all do, so that each item reads as if the list were not there.
    fn visit_children(&mut self, node: &Node, index: usize) {
        let mut previous = self.page.flow[index].previous;
        let items_of_list = matches!(node, Node::List(_));
        for child in node.children().map(Vec::as_slice).unwrap_or_default() {
            let child_index = self.visit(child, Some(node), previous);
            if !items_of_list {
                previous = Some(child_index);
            }
        }
    }

    fn phrasing(&mut self, children: &[Node], block: usize, table: Option<usize>) {
        if transform_fires(children, self.source) {
            self.positionless = true;
        }
        phrasing_comments(children, &mut self.context(block, table));
    }

    fn context(&mut self, block: usize, table: Option<usize>) -> CommentContext<'_> {
        CommentContext {
            source: self.source,
            block,
            table,
            comments: &mut self.page.comments,
            unclosed: &mut self.page.unclosed,
            first_sentences: &mut self.page.first_sentences,
        }
    }

    /// The page-model block for one mdast block node.
    fn block_of(&mut self, node: &Node, parent: Option<&Node>, previous: Option<usize>) -> Block {
        let (start, end, from, to) = span(node);
        let from = task_paragraph_start(node, parent, self.source).unwrap_or(from);
        let end = mdast_end(node, self.source).unwrap_or(end);
        let kind = match node {
            Node::Heading(h) => BlockKind::Heading {
                text: content_range(&h.children, self.source)
                    .map(|(a, b)| prose(slice(self.source, a, b)))
                    .unwrap_or_default(),
            },
            Node::Paragraph(_) => BlockKind::Paragraph { text: prose(slice(self.source, from, to)) },
            Node::List(list) => BlockKind::List { items: list.children.iter().map(list_item).collect() },
            Node::Table(table) => {
                let table = table_of(table, self.source);
                self.page.tables.push(table);
                BlockKind::Table(self.page.tables.len() - 1)
            }
            Node::Code(code) => {
                let info = [code.lang.as_deref(), code.meta.as_deref()]
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                BlockKind::Code { info, value: code.value.clone() }
            }
            _ => BlockKind::Other,
        };
        Block { start, end, kind, previous }
    }
}

/// Start and end lines and offsets of a node.
fn span(node: &Node) -> (usize, usize, usize, usize) {
    node.position().map_or((0, 0, 0, 0), |p| (p.start.line, p.end.line, p.start.offset, p.end.offset))
}

/// Where mdast-util-gfm-task-list-item starts the first paragraph of a task item when its
/// content opens with something other than text: at the `[x]` check, which markdown-rs
/// leaves out of the paragraph.
fn task_paragraph_start(node: &Node, parent: Option<&Node>, source: &str) -> Option<usize> {
    if !is_task_paragraph_without_text(node, parent) {
        return None;
    }
    let start = node.position()?.start.offset;
    let before = source.get(..start)?.trim_end_matches([' ', '\t']);
    let check = before.len().checked_sub(3)?;
    matches!(before.get(check..)?, "[x]" | "[X]" | "[ ]").then_some(check)
}

/// The node is the first paragraph of a task item, and does not open with text.
fn is_task_paragraph_without_text(node: &Node, parent: Option<&Node>) -> bool {
    let (Some(Node::ListItem(item)), Node::Paragraph(paragraph)) = (parent, node) else {
        return false;
    };
    if item.checked.is_none() {
        return false;
    }
    let first = item.children.iter().find(|c| matches!(c, Node::Paragraph(_)));
    first.is_some_and(|first| std::ptr::eq(first, node))
        && !matches!(paragraph.children.first(), Some(Node::Text(_)))
}

/// The end line mdast-util-from-markdown gives where markdown-rs differs: a list item or a
/// footnote definition ends with its last child, before trailing blank lines; a list ends
/// before trailing line endings.
fn mdast_end(node: &Node, source: &str) -> Option<usize> {
    let children = match node {
        Node::ListItem(item) => Some(&item.children),
        Node::FootnoteDefinition(def) => Some(&def.children),
        _ => None,
    };
    match node {
        Node::ListItem(_) | Node::FootnoteDefinition(_) => match children.and_then(|c| c.last()) {
            Some(last) => mdast_end(last, source).or(last.position().map(|p| p.end.line)),
            None => trimmed_end(node, source),
        },
        Node::List(_) => trimmed_end(node, source),
        _ => None,
    }
}

fn trimmed_end(node: &Node, source: &str) -> Option<usize> {
    let p = node.position()?;
    let before = source.get(..p.end.offset)?;
    let kept = before.trim_end_matches([' ', '\t', '\n']);
    Some(p.end.line - before[kept.len()..].matches('\n').count())
}

fn list_item(item: &Node) -> ListItem {
    let paragraph = item.children().and_then(|c| c.iter().find(|child| matches!(child, Node::Paragraph(_))));
    ListItem { line: span(item).0, value: paragraph.map(item_value).unwrap_or_default() }
}

fn table_of(table: &MdTable, source: &str) -> Table {
    let mut rows = table.children.iter();
    let header = rows
        .next()
        .and_then(Node::children)
        .map(|cells| cells.iter().map(|c| cell_raw(c, source)).collect())
        .unwrap_or_default();
    let rows = rows
        .map(|row| TableRow {
            line: span(row).0,
            cells: row
                .children()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| TableCell { raw: cell_raw(cell, source), values: cell_values(cell) })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    Table { header, rows }
}

// Cases runners receive cells as written, so a cell keeps its code spans and comments;
// only the pipe escape, which exists because of the table itself, is undone.
fn cell_raw(cell: &Node, source: &str) -> String {
    let children = match cell {
        Node::TableCell(MdCell { children, .. }) => children.as_slice(),
        _ => &[],
    };
    content_range(children, source)
        .map(|(a, b)| trim(&slice(source, a, b).replace("\\|", "|")).to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
