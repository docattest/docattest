//! Text from mdast nodes: prose for claims, values for comparison.

use markdown::mdast::Node;

use crate::text::{collapse_spaces, is_js_space, trim};

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

/// Prose for claims: HTML comments removed ([`without_comments`]), whitespace collapsed.
/// Markup is kept.
pub fn prose(source: &str) -> String {
    trim(&collapse_spaces(&without_comments(source))).to_string()
}

/// `source` without its HTML comments, as a reader sees it: a comment and the whitespace
/// around it become one space between words, and nothing at either end or before closing
/// punctuation, so a directive between a word and its comma leaves the two together.
fn without_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for (start, end) in comment_ranges(source) {
        out.push_str(&source[at..start]);
        out.truncate(out.trim_end_matches(is_js_space).len());
        let rest = source[end..].trim_start_matches(is_js_space);
        at = source.len() - rest.len();
        if !out.is_empty() && rest.chars().next().is_some_and(|c| !closes(c)) {
            out.push(' ');
        }
    }
    out.push_str(&source[at..]);
    out
}

/// Punctuation that closes what comes before it, so no space goes in front of it.
fn closes(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
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

/// The values of a table cell as a column fact compares them: every code span in it, as a
/// cell may group several (`` `A`, `B` ``); else its [`item_value`].
pub fn cell_values(node: &Node) -> Vec<String> {
    let spans = code_spans(node);
    if spans.is_empty() {
        return vec![item_value(node)];
    }
    spans.into_iter().map(|span| trim(span).to_string()).collect()
}

/// Every inline code span inside a node, depth-first.
fn code_spans(node: &Node) -> Vec<&str> {
    match node {
        Node::InlineCode(c) => vec![c.value.as_str()],
        _ => node.children().map_or_else(Vec::new, |children| children.iter().flat_map(code_spans).collect()),
    }
}

/// A value as docattest compares it: the first code span; else a bold lead term, as in
/// `**SIMPLE**: runs once`; else the visible text.
pub fn item_value(node: &Node) -> String {
    if let Some(code) = code_spans(node).first() {
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

    #[test]
    fn a_removed_comment_leaves_one_space_between_words_and_none_before_punctuation() {
        assert_eq!(without_comments("is `acme` <!-- fact: USER -->, always."), "is `acme`, always.");
        assert_eq!(without_comments("`2` <!-- fact: N -->"), "`2`");
        assert_eq!(without_comments("x <!-- a --> <!-- b -->. y"), "x. y");
        assert_eq!(without_comments("a  <!-- a -->\n b (c)"), "a b (c)");
        assert_eq!(without_comments("a <!-- a --> (c)"), "a (c)");
        assert_eq!(without_comments("  <!-- a -->  a  "), "a  ");
        assert_eq!(prose("is `acme` <!-- fact: USER --> , always."), "is `acme`, always.");
    }

    /// protocol/test-data/code-spans.json is the contract every language's cell helper is tested
    /// against; this holds the core to it, so the table cannot drift from what a fact over a
    /// column compares. It is read at run time: the crate as published carries no protocol/.
    #[test]
    fn a_cells_code_spans_are_the_shared_tables() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../protocol/test-data/code-spans.json");
        let table: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for case in table["cases"].as_array().unwrap() {
            let cell = case["cell"].as_str().unwrap().replace('|', "\\|");
            let (_, tree) =
                crate::markdown::parse::parse_tree(&format!("| c |\n|---|\n| {cell} |\n")).unwrap();
            let got: Vec<&str> =
                first_body_cell(&tree).map(code_spans).unwrap_or_default().into_iter().map(trim).collect();
            let want: Vec<&str> =
                case["spans"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
            assert_eq!(got, want, "{}: {:?}", case["name"], case["cell"]);
        }
    }

    /// The first cell of the table's second row: its body, below the header.
    fn first_body_cell(node: &Node) -> Option<&Node> {
        if let Node::Table(table) = node {
            return table.children.get(1)?.children()?.first();
        }
        node.children()?.iter().find_map(first_body_cell)
    }
}
