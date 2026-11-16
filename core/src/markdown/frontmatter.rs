//! Frontmatter: whether a page freezes itself with `docattest: false`.
//!
//! A page is frozen when its frontmatter is one YAML 1.2 document (core schema, unique
//! keys) whose top level is a mapping with `docattest` set to `false`. Invalid YAML leaves
//! the page living: failing to freeze is the safe direction. Only that one question is
//! answered here, from yaml-rust2's events.

use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::TScalarStyle;

enum Yaml {
    Scalar { value: String, plain: bool, tag: Option<String> },
    Seq,
    Map(Vec<(Yaml, Yaml)>),
    Alias,
}

/// A scalar key as resolved by the core schema: keys are unique by resolved value, not by
/// spelling.
#[derive(PartialEq)]
enum Resolved {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

/// Whether frontmatter text is valid YAML holding a mapping with `docattest: false`.
pub fn freezes(text: &str) -> bool {
    match parse(text) {
        Some(Yaml::Map(entries)) => entries.iter().any(|(key, value)| {
            matches!(key, Yaml::Scalar { value, tag: None, .. } if value == "docattest") && is_false(value)
        }),
        _ => false,
    }
}

fn is_false(node: &Yaml) -> bool {
    match node {
        Yaml::Scalar { value, plain: true, tag: None } => is_false_word(value),
        Yaml::Scalar { value, tag: Some(tag), .. } => {
            (tag == "tag:yaml.org,2002:bool" || tag == "!!bool") && is_false_word(value)
        }
        _ => false,
    }
}

fn is_false_word(value: &str) -> bool {
    matches!(value, "false" | "False" | "FALSE")
}

/// The single document in `text`, or None when the text is invalid YAML, holds several
/// documents, has a duplicate key, or is empty.
fn parse(text: &str) -> Option<Yaml> {
    let mut parser = Parser::new_from_str(text);
    let mut tree = Tree { stack: Vec::new(), documents: 0, root: None };
    loop {
        let (event, _) = parser.next_token().ok()?;
        if matches!(event, Event::StreamEnd) {
            break;
        }
        if let Some(node) = tree.node_of(event)? {
            tree.attach(node);
        }
    }
    tree.root
}

enum Frame {
    Seq,
    Map(Vec<(Yaml, Yaml)>, Option<Yaml>),
}

/// The document as its events build it.
struct Tree {
    /// The collections open around the next node.
    stack: Vec<Frame>,
    documents: usize,
    root: Option<Yaml>,
}

impl Tree {
    /// The node an event completes (`Some(None)` for an event that completes none); `None`
    /// when the text is not one document of unique keys.
    fn node_of(&mut self, event: Event) -> Option<Option<Yaml>> {
        match event {
            Event::DocumentStart => {
                self.documents += 1;
                if self.documents > 1 {
                    return None;
                }
                Some(None)
            }
            Event::StreamStart | Event::StreamEnd | Event::DocumentEnd | Event::Nothing => Some(None),
            Event::Scalar(value, style, _, tag) => Some(Some(Yaml::Scalar {
                value,
                plain: style == TScalarStyle::Plain,
                tag: tag.map(|t| format!("{}{}", t.handle, t.suffix)),
            })),
            Event::Alias(_) => Some(Some(Yaml::Alias)),
            Event::SequenceStart(..) => {
                self.stack.push(Frame::Seq);
                Some(None)
            }
            Event::MappingStart(..) => {
                self.stack.push(Frame::Map(Vec::new(), None));
                Some(None)
            }
            Event::SequenceEnd => self.close_sequence().map(Some),
            Event::MappingEnd => self.close_mapping().map(Some),
        }
    }

    fn close_sequence(&mut self) -> Option<Yaml> {
        match self.stack.pop() {
            Some(Frame::Seq) => Some(Yaml::Seq),
            _ => None,
        }
    }

    fn close_mapping(&mut self) -> Option<Yaml> {
        match self.stack.pop() {
            Some(Frame::Map(entries, None)) if !has_duplicates(&entries) => Some(Yaml::Map(entries)),
            _ => None,
        }
    }

    /// Puts a node in the collection around it: a key, a key's value, or the root.
    fn attach(&mut self, node: Yaml) {
        match self.stack.last_mut() {
            None => self.root = Some(node),
            Some(Frame::Seq) => {}
            Some(Frame::Map(entries, pending)) => match pending.take() {
                None => *pending = Some(node),
                Some(key) => entries.push((key, node)),
            },
        }
    }
}

fn has_duplicates(entries: &[(Yaml, Yaml)]) -> bool {
    let keys: Vec<Option<Resolved>> = entries.iter().map(|(k, _)| resolve(k)).collect();
    keys.iter()
        .enumerate()
        .any(|(i, a)| a.as_ref().is_some_and(|a| keys[..i].iter().flatten().any(|b| a == b)))
}

// The YAML 1.2 core schema, as far as key equality needs it.
fn resolve(node: &Yaml) -> Option<Resolved> {
    let Yaml::Scalar { value, plain, tag } = node else {
        return None;
    };
    if !plain || tag.is_some() {
        return Some(Resolved::String(value.clone()));
    }
    Some(match value.as_str() {
        "" | "~" | "null" | "Null" | "NULL" => Resolved::Null,
        "true" | "True" | "TRUE" => Resolved::Bool(true),
        "false" | "False" | "FALSE" => Resolved::Bool(false),
        _ => match number(value) {
            Some(n) => Resolved::Number(n),
            None => Resolved::String(value.clone()),
        },
    })
}

fn number(value: &str) -> Option<f64> {
    let (sign, digits) = match value.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, value.strip_prefix('+').unwrap_or(value)),
    };
    if let Some(hex) = value.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16).ok().map(|n| n as f64);
    }
    if let Some(oct) = value.strip_prefix("0o") {
        return u64::from_str_radix(oct, 8).ok().map(|n| n as f64);
    }
    if let Some(special) = special_float(digits, sign) {
        return Some(special);
    }
    if !looks_decimal(digits) {
        return None;
    }
    digits.parse::<f64>().ok().map(|n| sign * n)
}

/// `.inf` and `.nan`, in the spellings the core schema has.
fn special_float(digits: &str, sign: f64) -> Option<f64> {
    match digits {
        ".inf" | ".Inf" | ".INF" => Some(sign * f64::INFINITY),
        ".nan" | ".NaN" | ".NAN" => Some(f64::NAN),
        _ => None,
    }
}

fn looks_decimal(digits: &str) -> bool {
    !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        && digits.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freezing() {
        assert!(freezes("docattest: false\ntitle: x"));
        assert!(freezes("docattest: False"));
        assert!(!freezes("docattest: 'false'"));
        assert!(!freezes("docattest: no"));
        assert!(!freezes(": : :"));
        assert!(!freezes("docattest: false\ndocattest: false"));
        assert!(!freezes(""));
        assert!(!freezes("- docattest: false"));
    }
}
