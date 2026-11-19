//! HTML comments in a page: where they are and what surrounds them.

use markdown::mdast::{Html, Node};

use super::model::{Comment, FirstSentence, Keyword, Leaf, LeafKind, Unclosed};
use super::text::{comment_ranges, content_range, ends_line_past_comments, ends_sentence, prose, slice};
use crate::text::{is_js_space, is_word, lines, trim};

/// Where found comments go, and the context they were found in.
pub struct CommentContext<'a> {
    pub source: &'a str,
    /// Index into the page's flow of the block whose text is searched.
    pub block: usize,
    pub table: Option<usize>,
    pub comments: &'a mut Vec<Comment>,
    pub unclosed: &'a mut Vec<Unclosed>,
    pub first_sentences: &'a mut Vec<FirstSentence>,
}

/// `/<!--\s*(fact|cases|tests?(?=\s*:))\b/.exec(text)`: the byte index of the opening and its
/// keyword. `test` and `tests` open a directive only with their colon; `<!-- testing` is prose.
pub fn opening(text: &str) -> Option<(usize, Keyword)> {
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|t| t.find("<!--")) {
        let index = from + found;
        let after = &text[index + 4..];
        let rest = after.trim_start_matches(is_js_space);
        if let Some(keyword) = keyword_at(rest) {
            return Some((index, keyword));
        }
        from = index + 1;
    }
    None
}

fn keyword_at(rest: &str) -> Option<Keyword> {
    let opens = |keyword: &Keyword| {
        let Some(tail) = rest.strip_prefix(keyword.as_str()) else { return false };
        match keyword.needs_colon() {
            true => tail.trim_start_matches(is_js_space).starts_with(':'),
            false => !tail.chars().next().is_some_and(is_word),
        }
    };
    Keyword::ALL.into_iter().find(opens)
}

fn line_of(node: &Node) -> usize {
    node.position().map_or(0, |p| p.start.line)
}

/// Comments inside a paragraph, heading or table cell, with the leaves before each; and, for
/// a paragraph or heading, where its first sentence ends (its text's end when none does).
pub fn phrasing_comments(children: &[Node], ctx: &mut CommentContext) {
    let range = content_range(children, ctx.source);
    let start = range.map_or(0, |r| r.0);
    let mut leaves = LeavesBefore::default();
    for child in children {
        visit_phrasing(child, start, &mut leaves, ctx);
    }
    if let Some(range) = range.filter(|_| reads_first_sentence(ctx)) {
        let line = children.last().and_then(Node::position).map_or(0, |p| p.end.line);
        push_first_sentence(ctx, range, line, leaves.since_comment);
    }
}

/// The leaves read so far in a phrasing container, as the next comment sees them.
#[derive(Default)]
struct LeavesBefore {
    /// Since the previous comment.
    since_comment: Vec<Leaf>,
    /// What the previous comment saw.
    seen_by_previous: Option<Vec<Leaf>>,
}

impl LeavesBefore {
    /// The leaves since the previous comment; or, when only whitespace parts the two, what
    /// that comment saw: comments side by side are one group, and each sees what is before
    /// the group, whatever their order.
    fn seen_by_comment(&self) -> Vec<Leaf> {
        let blank = self.since_comment.iter().all(|l| l.kind == LeafKind::Text && trim(&l.value).is_empty());
        match &self.seen_by_previous {
            Some(seen) if blank => seen.clone(),
            _ => self.since_comment.clone(),
        }
    }

    fn comment_found(&mut self, seen: Vec<Leaf>) {
        self.since_comment.clear();
        self.seen_by_previous = Some(seen);
    }
}

fn visit_phrasing(node: &Node, start: usize, leaves: &mut LeavesBefore, ctx: &mut CommentContext) {
    match node {
        Node::Text(t) => visit_text(node, &t.value, start, &mut leaves.since_comment, ctx),
        Node::InlineCode(c) => {
            let leaf = Leaf { kind: LeafKind::Code, value: c.value.clone(), line: line_of(node) };
            leaves.since_comment.push(leaf);
        }
        Node::Html(html) => {
            let seen = leaves.seen_by_comment();
            if inline_comment(html, start, &seen, ctx) {
                leaves.comment_found(seen);
            }
        }
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    visit_phrasing(child, start, leaves, ctx);
                }
            }
        }
    }
}

/// A text node, one leaf per line; the first sentence of its block may end in it.
fn visit_text(node: &Node, value: &str, start: usize, before: &mut Vec<Leaf>, ctx: &mut CommentContext) {
    let line = line_of(node);
    let texts = lines(value);
    for (k, text) in texts.iter().enumerate() {
        if let Some((_, keyword)) = opening(text) {
            push_unclosed(ctx, keyword, line + k);
        }
        let after = if k + 1 < texts.len() { Some('\n') } else { char_after(node, ctx.source) };
        if let Some(cut) = sentence_end_in(text, after).filter(|_| reads_first_sentence(ctx)) {
            let mut sentence = before.clone();
            sentence.push(Leaf { kind: LeafKind::Text, value: text[..cut].to_string(), line: line + k });
            push_first_sentence(ctx, (start, sentence_end_offset(node, ctx.source)), line + k, sentence);
        }
        before.push(Leaf { kind: LeafKind::Text, value: text.to_string(), line: line + k });
    }
}

/// Whether the first sentence of the block being read is still to be found: a table cell is
/// not a sentence a directive above it attests.
fn reads_first_sentence(ctx: &CommentContext) -> bool {
    ctx.table.is_none() && !has_first_sentence(ctx)
}

fn has_first_sentence(ctx: &CommentContext) -> bool {
    ctx.first_sentences.last().is_some_and(|s| s.block == ctx.block)
}

/// Just after the first `.`, `!` or `?` in `text` that ends a sentence ([`ends_sentence`]);
/// `after` is the character that follows `text`.
fn sentence_end_in(text: &str, after: Option<char>) -> Option<usize> {
    let mut chars = text.char_indices().peekable();
    while let Some((k, c)) = chars.next() {
        if ends_sentence(c, chars.peek().map(|&(_, next)| next).or(after)) {
            return Some(k + c.len_utf8());
        }
    }
    None
}

/// The character of the page right after a node.
fn char_after(node: &Node, source: &str) -> Option<char> {
    let end = node.position()?.end.offset;
    source.get(end..)?.chars().next()
}

/// Where in the page the first sentence of a text node ends: found again in its source, where
/// an escape or a character reference reads differently from its value; the node's end if
/// the two disagree.
fn sentence_end_offset(node: &Node, source: &str) -> usize {
    let Some(p) = node.position() else { return 0 };
    let (from, to) = (p.start.offset, p.end.offset);
    sentence_end_in(slice(source, from, to), char_after(node, source)).map_or(to, |cut| from + cut)
}

/// Record the first sentence of the block being read: from `range.0` to `range.1` in the page,
/// ending on `line`.
fn push_first_sentence(ctx: &mut CommentContext, range: (usize, usize), line: usize, before: Vec<Leaf>) {
    ctx.first_sentences.push(FirstSentence {
        block: ctx.block,
        line,
        ends_line: ends_line_past_comments(ctx.source.get(range.1..).unwrap_or("")),
        lead: prose(slice(ctx.source, range.0, range.1)),
        before,
    });
}

fn inline_comment(node: &Html, start: usize, before: &[Leaf], ctx: &mut CommentContext) -> bool {
    // /^<!--([\s\S]*?)-->$/
    let value = node.value.as_str();
    if value.len() < 7 || !value.starts_with("<!--") || !value.ends_with("-->") {
        return false;
    }
    let Some(position) = node.position.as_ref() else {
        return false;
    };
    let (from, to) = (position.start.offset, position.end.offset);
    ctx.comments.push(Comment {
        body: value[4..value.len() - 3].to_string(),
        line: position.start.line,
        end_line: position.end.line,
        ends_line: ends_line_past_comments(ctx.source.get(to..).unwrap_or("")),
        block: ctx.block,
        lead: prose(slice(ctx.source, start, from)),
        before: before.to_vec(),
        table: ctx.table,
        alone: false,
    });
    true
}

/// Comments in an HTML block: own-line directives, possibly several, possibly multi-line.
pub fn flow_comments(node: &Html, ctx: &mut CommentContext) {
    let first = node.position.as_ref().map_or(0, |p| p.start.line);
    let value = node.value.as_str();
    let line_at = |index: usize| first + value[..index].matches('\n').count();
    let alone = prose(value).is_empty();
    let mut consumed = 0;
    for (start, end) in comment_ranges(value) {
        ctx.comments.push(Comment {
            body: value[start + 4..end - 3].to_string(),
            line: line_at(start),
            end_line: line_at(end),
            ends_line: ends_line_past_comments(&value[end..]),
            block: ctx.block,
            lead: prose(&value[..start]),
            before: Vec::new(),
            table: ctx.table,
            alone,
        });
        consumed = end;
    }
    let tail = &value[consumed..];
    if let Some((index, keyword)) = opening(tail) {
        push_unclosed(ctx, keyword, line_at(consumed + index));
    }
}

fn push_unclosed(ctx: &mut CommentContext, keyword: Keyword, line: usize) {
    ctx.unclosed.push(Unclosed { keyword, line, block: ctx.block, table: ctx.table });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openings() {
        assert_eq!(opening("x <!-- fact: N"), Some((2, Keyword::Fact)));
        assert_eq!(opening("<!--cases"), Some((0, Keyword::Cases)));
        assert_eq!(opening("<!-- facts"), None);
        assert_eq!(opening("<!-- testing"), None);
        assert_eq!(opening("<!-- test :"), Some((0, Keyword::Test)));
        assert_eq!(opening("<!-- test"), None);
        assert_eq!(opening("<!-- tests: a = b"), Some((0, Keyword::Tests)));
        assert_eq!(opening("<!-- tests pending"), None);
        assert_eq!(opening("<!-- x <!-- fact"), Some((7, Keyword::Fact)));
        assert_eq!(opening("<!-- factй"), Some((0, Keyword::Fact)));
    }
}
