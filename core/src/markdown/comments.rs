//! HTML comments in a page: where they are and what surrounds them.

use markdown::mdast::{Html, Node};

use super::model::{Comment, Keyword, Leaf, LeafKind, Unclosed};
use super::text::{comment_ranges, content_range, prose, slice};
use crate::text::{first_line, is_js_space, is_word, lines, trim};

/// Where found comments go, and the context they were found in.
pub struct CommentContext<'a> {
    pub source: &'a str,
    /// Index into the page's flow of the block whose text is searched.
    pub block: usize,
    pub table: Option<usize>,
    pub comments: &'a mut Vec<Comment>,
    pub unclosed: &'a mut Vec<Unclosed>,
}

/// `/<!--\s*(fact|cases|test(?=\s*:))\b/.exec(text)`: the byte index of the opening and its
/// keyword. `test` opens a directive only with its colon; `<!-- testing` is prose.
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
    let boundary = |tail: &str| !tail.chars().next().is_some_and(is_word);
    if let Some(tail) = rest.strip_prefix("fact")
        && boundary(tail)
    {
        return Some(Keyword::Fact);
    }
    if let Some(tail) = rest.strip_prefix("cases")
        && boundary(tail)
    {
        return Some(Keyword::Cases);
    }
    if let Some(tail) = rest.strip_prefix("test")
        && tail.trim_start_matches(is_js_space).starts_with(':')
    {
        return Some(Keyword::Test);
    }
    None
}

fn line_of(node: &Node) -> usize {
    node.position().map_or(0, |p| p.start.line)
}

/// Comments inside a paragraph, heading or table cell, with the leaves before each.
pub fn phrasing_comments(children: &[Node], ctx: &mut CommentContext) {
    let start = content_range(children, ctx.source).map_or(0, |r| r.0);
    let mut before: Vec<Leaf> = Vec::new();
    for child in children {
        visit_phrasing(child, start, &mut before, ctx);
    }
}

fn visit_phrasing(node: &Node, start: usize, before: &mut Vec<Leaf>, ctx: &mut CommentContext) {
    match node {
        Node::Text(t) => {
            let line = line_of(node);
            for (k, text) in lines(&t.value).into_iter().enumerate() {
                if let Some((_, keyword)) = opening(text) {
                    push_unclosed(ctx, keyword, line + k);
                }
                before.push(Leaf { kind: LeafKind::Text, value: text.to_string(), line: line + k });
            }
        }
        Node::InlineCode(c) => {
            before.push(Leaf { kind: LeafKind::Code, value: c.value.clone(), line: line_of(node) })
        }
        Node::Html(html) => {
            if inline_comment(html, start, before, ctx) {
                before.clear();
            }
        }
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    visit_phrasing(child, start, before, ctx);
                }
            }
        }
    }
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
    let rest = first_line(ctx.source.get(to..).unwrap_or(""));
    ctx.comments.push(Comment {
        body: value[4..value.len() - 3].to_string(),
        line: position.start.line,
        end_line: position.end.line,
        ends_line: trim(rest).is_empty(),
        block: ctx.block,
        lead: prose(slice(ctx.source, start, from)),
        before: before.to_vec(),
        table: ctx.table,
    });
    true
}

/// Comments in an HTML block: own-line directives, possibly several, possibly multi-line.
pub fn flow_comments(node: &Html, ctx: &mut CommentContext) {
    let first = node.position.as_ref().map_or(0, |p| p.start.line);
    let value = node.value.as_str();
    let line_at = |index: usize| first + value[..index].matches('\n').count();
    let mut consumed = 0;
    for (start, end) in comment_ranges(value) {
        let rest = first_line(&value[end..]);
        ctx.comments.push(Comment {
            body: value[start + 4..end - 3].to_string(),
            line: line_at(start),
            end_line: line_at(end),
            ends_line: trim(rest).is_empty(),
            block: ctx.block,
            lead: prose(&value[..start]),
            before: Vec::new(),
            table: ctx.table,
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
        assert_eq!(opening("<!-- x <!-- fact"), Some((7, Keyword::Fact)));
        assert_eq!(opening("<!-- factй"), Some((0, Keyword::Fact)));
    }
}
