//! Text from mdast nodes: prose for claims, values for comparison.

use markdown::mdast::Node;

use crate::text::{collapse_spaces, trim};

/// Every non-overlapping `<!--…-->` in `text` (lazy body), as byte ranges of the whole
/// comment, in order: `/<!--([\s\S]*?)-->/g`.
pub fn comment_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(open) = text.get(at..).and_then(|t| t.find("<!--")) {
        let start = at + open;
        let Some(close) = text.get(start + 4..).and_then(|t| t.find("-->")) else {
            break;
        };
        let end = start + 4 + close + 3;
        out.push((start, end));
        at = end;
    }
    out
}

/// Prose for claims: HTML comments removed, whitespace collapsed. Markup is kept.
pub fn prose(source: &str) -> String {
    let mut without = String::with_capacity(source.len());
    let mut at = 0;
    for (start, end) in comment_ranges(source) {
        without.push_str(&source[at..start]);
        without.push(' ');
        at = end;
    }
    without.push_str(&source[at..]);
    trim(&collapse_spaces(&without)).to_string()
}

/// The text a reader sees in a node: text and code values, without HTML.
fn plain_text(node: &Node, out: &mut String) {
    match node {
        Node::Text(t) => out.push_str(&t.value),
        Node::InlineCode(c) => out.push_str(&c.value),
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    plain_text(child, out);
                }
            }
        }
    }
}

/// The first inline code span inside a node, depth-first.
fn first_code(node: &Node) -> Option<&str> {
    if let Node::InlineCode(c) = node {
        return Some(&c.value);
    }
    node.children()?.iter().find_map(first_code)
}

fn bold_lead(node: &Node) -> Option<String> {
    let first = node.children()?.first()?;
    if let Node::Strong(_) = first {
        let mut out = String::new();
        plain_text(first, &mut out);
        Some(out)
    } else {
        None
    }
}

/// A value as docattest compares it: the first code span; else a bold lead term, as in
/// `**SIMPLE**: runs once`; else the visible text.
pub fn item_value(node: &Node) -> String {
    if let Some(code) = first_code(node) {
        return trim(code).to_string();
    }
    if let Some(bold) = bold_lead(node) {
        return trim(&bold).to_string();
    }
    let mut out = String::new();
    plain_text(node, &mut out);
    trim(&out).to_string()
}

/// Start and end offsets of a node's content, or None for an empty node.
pub fn content_range(children: &[Node], source: &str) -> Option<(usize, usize)> {
    let first = start_offset(children.first()?, source)?;
    let last = children.last()?.position()?.end.offset;
    Some((first, last))
}

/// A node's start offset as mdast-util-from-markdown gives it: markdown-rs starts a text
/// node that opens with a character escape after the backslash, mdast-util-from-markdown at
/// it.
pub fn start_offset(node: &Node, source: &str) -> Option<usize> {
    let start = node.position()?.start.offset;
    let bytes = source.as_bytes();
    let escaped = matches!(node, Node::Text(_))
        && start > 0
        && bytes.get(start - 1) == Some(&b'\\')
        && bytes.get(start).is_some_and(u8::is_ascii_punctuation);
    Some(if escaped { start - 1 } else { start })
}

/// Whether `c`, followed by `next` (None at the end of the text), ends a sentence:
/// `/[.!?](?=\s|$)/`. A code span is never `next`'s whitespace, so a full stop right
/// before one does not end a sentence.
pub fn ends_sentence(c: char, next: Option<char>) -> bool {
    matches!(c, '.' | '!' | '?') && next.is_none_or(crate::text::is_js_space)
}

/// Positions in `text` just after each sentence's closing `.`, `!` or `?`: where whitespace
/// or the end of the text follows it ([`ends_sentence`]).
pub fn sentence_ends(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.char_indices()
        .map(|(k, c)| (c, k + c.len_utf8()))
        .filter(|&(c, end)| ends_sentence(c, text[end..].chars().next()))
        .map(|(_, end)| end)
}

/// `source.slice(from, to)`, empty where the range is not valid.
pub fn slice(source: &str, from: usize, to: usize) -> &str {
    source.get(from..to).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_drops_comments_and_collapses_whitespace() {
        assert_eq!(prose("a <!-- x --> b\n  c"), "a b c");
        assert_eq!(prose("a <!-- x"), "a <!-- x");
        assert_eq!(prose("<!-- a --><!-- b -->x"), "x");
        assert_eq!(prose("<!-->x-->y"), "y");
    }
}
