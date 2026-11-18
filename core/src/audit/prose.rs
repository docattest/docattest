//! Sentences of prose that no directive binds, with the code spans and bare numbers in them:
//! values a page copied into a sentence rather than into a list, a table or a block.
//!
//! A sentence ends at `.`, `!` or `?` before whitespace or the end of its paragraph, as
//! wherever docattest reads sentences. A directive binds the sentence it sits in, or the one
//! before it when nothing but whitespace comes between them; a sentence it binds is left out.
//! Conservative on purpose, as for lists: any comment that opens like a directive binds.

use markdown::mdast::Node;

use crate::attestations::literals::literals;
use crate::markdown::comments::opening;
use crate::markdown::parse::parse_tree;
use crate::markdown::text::{ends_sentence, slice};
use crate::text::{is_js_space, lines, trim};

/// A literal that a directive placed right after it binds alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    pub line: usize,
    /// The value, as a fact compares it.
    pub value: String,
    /// As the page writes it: a code span with its backticks, a number as it is.
    pub written: String,
}

/// A sentence no directive binds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SentenceCandidate {
    /// The line the sentence ends on, where a directive after it goes.
    pub line: usize,
    /// Its code spans, in order: an enumeration when two or more differ.
    pub spans: Vec<String>,
    /// Its code spans and bare numbers, in order: a directive right after one compares that
    /// one with a value that is not a list.
    pub values: Vec<Literal>,
}

/// The unbound sentences of a page's paragraphs, in order, except in paragraphs that start
/// inside one of `skipped` (line ranges of blocks a directive binds), and except the first
/// sentence of a paragraph that starts on a line of `opened` (a directive above attests it).
pub fn sentences_of(text: &str, skipped: &[(usize, usize)], opened: &[usize]) -> Vec<SentenceCandidate> {
    let Some((source, tree)) = parse_tree(text) else {
        return Vec::new();
    };
    let mut paragraphs = Vec::new();
    paragraphs_in(&tree, &mut paragraphs);
    paragraphs
        .into_iter()
        .filter(|paragraph| {
            let line = line_of(paragraph);
            !skipped.iter().any(|(start, end)| (*start..=*end).contains(&line))
        })
        .flat_map(|paragraph| {
            let opening = Reading { bound: opened.contains(&line_of(paragraph)), ..Reading::default() };
            sentences_in(paragraph, &source, opening)
        })
        .collect()
}

fn line_of(node: &Node) -> usize {
    node.position().map_or(0, |p| p.start.line)
}

fn paragraphs_in<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    if let Node::Paragraph(_) = node {
        out.push(node);
        return;
    }
    for child in node.children().into_iter().flatten() {
        paragraphs_in(child, out);
    }
}

/// A paragraph as a reader meets it, in order.
enum Token<'a> {
    Text {
        value: &'a str,
        line: usize,
    },
    Code {
        value: &'a str,
        written: &'a str,
        line: usize,
    },
    Directive,
    /// Anything else that is not whitespace: other HTML, an image.
    Other,
}

fn tokens<'a>(node: &'a Node, source: &'a str, out: &mut Vec<Token<'a>>) {
    let line = line_of(node);
    match node {
        Node::Text(t) => {
            out.push(Token::Text { value: &t.value, line });
            if opening(&t.value).is_some() {
                out.push(Token::Directive);
            }
        }
        Node::InlineCode(c) => {
            let written = node.position().map_or("", |p| slice(source, p.start.offset, p.end.offset));
            out.push(Token::Code { value: &c.value, written, line });
        }
        Node::Html(html) if opening(&html.value).is_some() => out.push(Token::Directive),
        Node::Break(_) => out.push(Token::Text { value: "\n", line }),
        _ => match node.children() {
            Some(children) => children.iter().for_each(|child| tokens(child, source, out)),
            None => out.push(Token::Other),
        },
    }
}

/// The first character a token shows, for the sentence end before it.
fn first_char(token: Option<&Token>) -> Option<char> {
    match token? {
        Token::Text { value, .. } => value.chars().next(),
        Token::Code { written, .. } => written.chars().next(),
        Token::Directive => Some('<'),
        Token::Other => Some('!'),
    }
}

/// A sentence being read, and whether a directive binds it.
#[derive(Default)]
struct Reading {
    sentence: SentenceCandidate,
    /// Something other than whitespace was read.
    seen: bool,
    bound: bool,
}

/// The sentences of a paragraph, read from `opening`, the state its first sentence starts in.
fn sentences_in(paragraph: &Node, source: &str, opening: Reading) -> Vec<SentenceCandidate> {
    let mut found = Vec::new();
    for child in paragraph.children().into_iter().flatten() {
        tokens(child, source, &mut found);
    }
    let mut done: Vec<Reading> = Vec::new();
    let mut current = opening;
    for (k, token) in found.iter().enumerate() {
        match token {
            Token::Text { value, line } => {
                read_text(value, *line, first_char(found.get(k + 1)), &mut current, &mut done)
            }
            Token::Code { value, written, line } => current.code(value, written, *line),
            Token::Directive => match done.last_mut() {
                Some(previous) if !current.seen => previous.bound = true,
                _ => current.bound = true,
            },
            Token::Other => current.seen = true,
        }
    }
    done.push(current);
    done.into_iter().filter(|r| r.seen && !r.bound).map(|r| r.sentence).collect()
}

/// Read a run of text, ending sentences inside it; `following` is the first character after it.
fn read_text(
    value: &str,
    line: usize,
    following: Option<char>,
    current: &mut Reading,
    done: &mut Vec<Reading>,
) {
    let mut from = 0;
    let mut at_line = line;
    let mut chars = value.char_indices().peekable();
    while let Some((k, c)) = chars.next() {
        let next = chars.peek().map(|(_, n)| *n).or(following);
        if !ends_sentence(c, next) {
            continue;
        }
        let end = k + c.len_utf8();
        current.text(&value[from..end], at_line);
        at_line += value[from..end].matches('\n').count();
        done.push(std::mem::take(current));
        from = end;
    }
    current.text(&value[from..], at_line);
}

impl Reading {
    fn text(&mut self, piece: &str, line: usize) {
        for (k, text) in lines(piece).into_iter().enumerate() {
            if text.chars().all(is_js_space) {
                continue;
            }
            self.seen = true;
            self.sentence.line = line + k;
            for number in literals(text) {
                let literal = Literal { line: line + k, written: number.clone(), value: number };
                self.sentence.values.push(literal);
            }
        }
    }

    fn code(&mut self, value: &str, written: &str, line: usize) {
        let value = trim(value).to_string();
        let literal = Literal { line, value: value.clone(), written: written.to_string() };
        self.sentence.values.push(literal);
        self.sentence.spans.push(value);
        self.sentence.line = line;
        self.seen = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Vec<SentenceCandidate> {
        sentences_of(text, &[], &[])
    }

    fn literal(line: usize, value: &str, written: &str) -> Literal {
        Literal { line, value: value.into(), written: written.into() }
    }

    fn spans(found: &[SentenceCandidate]) -> Vec<Vec<&str>> {
        found.iter().map(|s| s.spans.iter().map(String::as_str).collect()).collect()
    }

    #[test]
    fn splits_a_paragraph_into_sentences_with_their_code_spans() {
        let found = read("Tag `python`, `py` or `pycon`. Set\n`DOCATTEST_CORE` first! Then `x`.\n");
        assert_eq!(spans(&found), [vec!["python", "py", "pycon"], vec!["DOCATTEST_CORE"], vec!["x"]]);
        assert_eq!(found.iter().map(|s| s.line).collect::<Vec<_>>(), [1, 2, 2]);
        assert_eq!(found[1].values, [literal(2, "DOCATTEST_CORE", "`DOCATTEST_CORE`")]);
    }

    #[test]
    fn a_full_stop_before_a_code_span_or_inside_one_ends_nothing() {
        let found = read("See v1.`x` and `a. b` here.\n");
        assert_eq!(spans(&found), [vec!["x", "a. b"]]);
    }

    #[test]
    fn values_are_the_code_spans_and_bare_numbers_in_order() {
        let found = read("Port 8080 or `PORT`, then 9090, `HOST` and 7070.\n");
        let values: Vec<&str> = found[0].values.iter().map(|l| l.value.as_str()).collect();
        assert_eq!(values, ["8080", "PORT", "9090", "HOST", "7070"]);
    }

    #[test]
    fn a_directive_binds_its_sentence_or_the_one_just_before() {
        let found = read(
            "Set `A`. <!-- fact: A --> Set `B` <!-- fact: B --> now. Set `C`.\n\nSet `D`.<!-- note -->\n",
        );
        assert_eq!(spans(&found), [vec!["C"], vec!["D"]]);
        assert_eq!(read("Use `a` and `b`. <!-- fact KEYS --> And `c`.\n").len(), 1);
    }

    #[test]
    fn reads_inside_emphasis_links_and_list_items_but_not_skipped_lines() {
        let text = "- **Use `a`** and [`b`](u).\n\n<!-- fact: X -->\n- `c` and `d`.\n";
        assert_eq!(spans(&read(text)), [vec!["a", "b"], vec!["c", "d"]]);
        assert_eq!(spans(&sentences_of(text, &[(4, 4)], &[])), [vec!["a", "b"]]);
    }

    #[test]
    fn keeps_the_written_form_of_a_code_span() {
        let found = read("Run ``a `b` c`` now.\n");
        assert_eq!(found[0].values, [literal(1, "a `b` c", "``a `b` c``")]);
    }

    #[test]
    fn looks_in_no_heading_table_or_fenced_block() {
        assert_eq!(read("# `A` and `B`\n\n| `x` |\n|---|\n| `y` |\n\n```\n`z`.\n```\n"), vec![]);
    }
}
