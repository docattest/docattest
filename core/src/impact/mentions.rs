//! Code spans that name an affected symbol without proving anything.

use std::collections::{HashMap, HashSet};

use markdown::mdast::Node;

use super::covered::Covered;
use super::{Found, ImpactReference};
use crate::markdown::parse::parse_tree;
use crate::markdown::text::{content_range, prose, sentence_ends, slice};

/// A page's inline code spans, read once: the page as parsed, and where each span sits.
pub struct PageSpans {
    source: String,
    tree: Node,
}

/// Read a page for its code spans; None for a page that cannot be parsed.
pub fn code_spans_of(text: &str) -> Option<PageSpans> {
    parse_tree(text).map(|(source, tree)| PageSpans { source, tree })
}

impl PageSpans {
    /// The text of every inline code span that sits in a paragraph, a heading or a table
    /// cell, as written.
    pub fn values(&self) -> Vec<&str> {
        let mut spans = Vec::new();
        code_spans(&self.tree, None, &mut spans);
        spans
            .into_iter()
            .filter_map(|(code, _)| match code {
                Node::InlineCode(span) => Some(span.value.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Inline code spans on a page that name a symbol, each with the sentence it sits in, except
/// on lines that already belong to an attestation of that symbol. Which symbol a span names
/// is the language's to say: `named` holds what it answered, by the span's text.
pub fn mentions_on(
    page: &str,
    spans: &PageSpans,
    named: &HashMap<&str, &str>,
    covered: &Covered,
) -> Vec<ImpactReference> {
    let mut found = Vec::new();
    code_spans(&spans.tree, None, &mut found);
    let mut seen: HashSet<(&str, usize)> = HashSet::new();
    let mut out = Vec::new();
    for (code, container) in found {
        let Node::InlineCode(span) = code else { continue };
        let Some(symbol) = named.get(span.value.as_str()).copied() else { continue };
        let Some(position) = code.position() else { continue };
        let line = position.start.line;
        if covered.covers(symbol, line) || !seen.insert((symbol, line)) {
            continue;
        }
        let claim = sentence(&spans.source, container, position.start.offset);
        out.push(ImpactReference {
            symbol: symbol.to_string(),
            page: page.to_string(),
            line,
            found: Found::Mention,
            claim,
        });
    }
    out
}

/// Every code span with the paragraph, heading or table cell holding it, in document order.
fn code_spans<'a>(node: &'a Node, container: Option<&'a Node>, out: &mut Vec<(&'a Node, &'a Node)>) {
    let holds = matches!(node, Node::Paragraph(_) | Node::Heading(_) | Node::TableCell(_));
    let here = if holds { Some(node) } else { container };
    if let (Node::InlineCode(_), Some(here)) = (node, here) {
        out.push((node, here));
    }
    if let Some(children) = node.children() {
        for child in children {
            code_spans(child, here, out);
        }
    }
}

/// The byte ranges of every code span inside `node`, relative to `from`.
fn mask(node: &Node, from: usize, text: &mut String) {
    if let Node::InlineCode(_) = node {
        if let Some(p) = node.position()
            && let (Some(a), Some(b)) = (p.start.offset.checked_sub(from), p.end.offset.checked_sub(from))
            && a <= b
            && b <= text.len()
            && text.is_char_boundary(a)
            && text.is_char_boundary(b)
        {
            text.replace_range(a..b, &"x".repeat(b - a));
        }
    } else if let Some(children) = node.children() {
        for child in children {
            mask(child, from, text);
        }
    }
}

// The sentence around the span: from the end of the previous sentence to the end of this
// one. Code spans are masked first, so a full stop inside one never ends a sentence.
// Offsets are bytes, not UTF-16 units; a masked span keeps its length in either, and every
// cut falls after an ASCII character, so the sentence is the same counted either way.
fn sentence(source: &str, container: &Node, code_from: usize) -> String {
    let children = container.children().map(Vec::as_slice).unwrap_or_default();
    let (from, to) = content_range(children, source).unwrap_or((0, 0));
    let mut text = slice(source, from, to).to_string();
    mask(container, from, &mut text);
    let at = code_from.saturating_sub(from);
    let ends: Vec<usize> = sentence_ends(&text).collect();
    let start = ends.iter().copied().filter(|&e| e <= at).max().unwrap_or(0);
    let end = ends.iter().copied().find(|&e| e > at).unwrap_or(text.len());
    prose(slice(source, from + start, from + end))
}

#[cfg(test)]
mod tests {
    use super::super::covered::covered_lines;
    use super::*;
    use crate::adapter::Language;
    use crate::adapter::fake::typescript;
    use crate::api::attestation::Kind;
    use crate::project::check::Reference;
    use crate::text::is_js_space;

    const PAGE: &str = "# Retry\n\nWrap the call in `retry`. It keeps trying. Then `retry(fn)` again.\n\nA successful call through `retry` returns its value.\n\n```ts check\nassert.equal(await retry(async () => 'ok'), 'ok');\n```\n\nSee `retry-after`, `retry.ts`, `RETRY.max` and `parse`.\n\n| name | use |\n|------|-----|\n| `retry` | the loop |\n\n## `RETRY` options";

    /// The mentions on a page, the stand-in language saying which symbol each span names.
    fn mentions(text: &str, symbols: &[&str], covered: &Covered) -> Vec<ImpactReference> {
        let spans = code_spans_of(text).unwrap();
        let values = spans.values();
        let answers = typescript().mentions(&values, symbols).unwrap();
        let named: HashMap<&str, &str> =
            values.iter().zip(&answers).filter_map(|(v, a)| Some((*v, a.as_deref()?))).collect();
        mentions_on("docs/r.md", &spans, &named, covered)
    }

    fn found(symbols: &[&str], proven: &[(&str, Vec<&Reference>)]) -> Vec<(String, usize, String)> {
        let covered = covered_lines(PAGE, proven);
        mentions(PAGE, symbols, &covered)
            .into_iter()
            .inspect(|m| assert!(m.found == Found::Mention && m.page == "docs/r.md"))
            .map(|m| (m.symbol, m.line, m.claim))
            .collect()
    }

    #[test]
    fn lists_code_spans_naming_a_symbol_with_their_sentence_outside_its_attestations() {
        let claim = "A successful call through `retry` returns its value.".into();
        let example = Reference { page: "docs/r.md".into(), line: 7, kind: Kind::Example, claim };
        let got = found(&["retry", "RETRY"], &[("retry", vec![&example]), ("RETRY", vec![])]);
        let expected = [
            ("retry", 3, "Wrap the call in `retry`."),
            ("RETRY", 11, "See `retry-after`, `retry.ts`, `RETRY.max` and `parse`."),
            ("retry", 15, "`retry`"),
            ("RETRY", 17, "`RETRY` options"),
        ];
        let expected: Vec<_> = expected.iter().map(|(s, l, c)| (s.to_string(), *l, c.to_string())).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn covers_the_paragraph_before_an_attestation_only_when_it_is_the_claim() {
        let fact = |claim: &str| Reference {
            page: "docs/r.md".into(),
            line: 5,
            kind: Kind::Fact,
            claim: claim.into(),
        };
        let lines = |reference: &Reference| -> Vec<usize> {
            found(&["retry"], &[("retry", vec![reference])]).into_iter().map(|(_, l, _)| l).collect()
        };
        let own_sentence = fact("A successful call through `retry`");
        assert!(lines(&own_sentence).contains(&3));
        let paragraph_before = fact("Wrap the call in `retry`. It keeps trying. Then `retry(fn)` again.");
        assert!(!lines(&paragraph_before).contains(&3));
    }

    #[test]
    fn covers_the_paragraph_below_a_directive_alone_on_the_line_above_it() {
        let page = "<!-- test: t.ts > \"retries\" -->\nWrap the call in `retry`.\n\nThen `retry` again.\n";
        let claim = "Wrap the call in `retry`.".into();
        let citation = Reference { page: "docs/r.md".into(), line: 1, kind: Kind::Test, claim };
        let covered = covered_lines(page, &[("retry", vec![&citation])]);
        let lines: Vec<usize> = mentions(page, &["retry"], &covered).into_iter().map(|m| m.line).collect();
        assert_eq!(lines, vec![4]);
    }

    #[test]
    fn lists_a_claim_paragraph_once_its_symbol_has_no_attestation_there() {
        let lines: Vec<usize> = found(&["retry"], &[]).into_iter().map(|m| m.1).collect();
        assert_eq!(lines, vec![3, 5, 15]);
    }

    #[test]
    fn spans_are_given_as_written_and_only_where_prose_holds_them() {
        let spans =
            code_spans_of("Use ` retry(fn) ` and `a`.\n\n- `b`\n\n```ts\n`not a span`\n```\n").unwrap();
        let values = spans.values();
        assert_eq!(
            values.iter().map(|v| v.trim_matches(is_js_space)).collect::<Vec<_>>(),
            ["retry(fn)", "a", "b"]
        );
    }

    #[test]
    fn sentences_end_at_a_stop_before_whitespace_and_never_inside_a_span() {
        let page = "Erst `a.b()`. Dann é `x. y` und `retry` hier! Ende.\n";
        let covered = covered_lines(page, &[]);
        let got = mentions(page, &["retry"], &covered);
        assert_eq!(got[0].claim, "Dann é `x. y` und `retry` hier!");
    }
}
