//! Doc comments: their prose, and the examples they hold.
//!
//! An example in a Rust doc comment is a fenced block, as rustdoc reads one: a block
//! with no info string, or with only words rustdoc knows, is Rust. `ignore` and
//! `compile_fail` blocks are left alone; `no_run` is compiled and not run; and
//! `should_panic` is passed on as the block's tag.

use crate::language::{SHOULD_PANIC, is_source, text};
use crate::protocol::{DocProse, Example, SourceText};
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ImplItem, Item, Lit, Meta, TraitItem};

/// The words of an info string that leave a block a Rust example.
pub const RUST_WORDS: [&str; 7] =
    ["rust", "no_run", SHOULD_PANIC, "edition2015", "edition2018", "edition2021", "edition2024"];

/// One doc comment: the item it documents, and its lines with their place in the file.
pub struct DocComment {
    /// The documented item's name, or its type's for a method; none for a file's own.
    pub owner: Option<String>,
    pub lines: Vec<(usize, String)>,
}

/// The name of an item that has one, and its attributes.
pub fn named(item: &Item) -> Option<(String, &[Attribute])> {
    Some(match item {
        Item::Const(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Static(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Fn(i) => (i.sig.ident.to_string(), &i.attrs[..]),
        Item::Mod(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Macro(i) => (i.ident.as_ref()?.to_string(), &i.attrs[..]),
        other => return named_type(other),
    })
}

/// The name of an item that declares a type, and its attributes.
fn named_type(item: &Item) -> Option<(String, &[Attribute])> {
    Some(match item {
        Item::Struct(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Enum(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Union(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Trait(i) => (i.ident.to_string(), &i.attrs[..]),
        Item::Type(i) => (i.ident.to_string(), &i.attrs[..]),
        _ => return None,
    })
}

/// The type an `impl` block is for, by its last name.
pub fn impl_owner(item: &syn::ItemImpl) -> Option<String> {
    match &*item.self_ty {
        syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// The lines of the doc comments among some attributes.
fn doc_lines(attrs: &[Attribute]) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    for attr in attrs {
        let Meta::NameValue(meta) = &attr.meta else { continue };
        let Expr::Lit(value) = &meta.value else { continue };
        let Lit::Str(written) = &value.lit else { continue };
        if !attr.path().is_ident("doc") {
            continue;
        }
        let first = attr.span().start().line;
        for (k, line) in written.value().split('\n').enumerate() {
            lines.push((first + k, line.strip_prefix(' ').unwrap_or(line).to_string()));
        }
    }
    lines
}

/// Every doc comment of a file, in order: its own, then each item's, with the items of
/// modules written in it, of `impl` blocks and of traits.
pub fn doc_comments(file: &syn::File) -> Vec<DocComment> {
    fn add(out: &mut Vec<DocComment>, owner: Option<String>, attrs: &[Attribute]) {
        let lines = doc_lines(attrs);
        if !lines.is_empty() {
            out.push(DocComment { owner, lines });
        }
    }
    fn walk(items: &[Item], out: &mut Vec<DocComment>) {
        for item in items {
            if let Some((name, attrs)) = named(item) {
                add(out, Some(name), attrs);
            }
            match item {
                Item::Mod(module) => walk(module.content.as_ref().map(|c| &c.1[..]).unwrap_or(&[]), out),
                Item::Impl(block) => block.items.iter().for_each(|inner| match inner {
                    ImplItem::Fn(f) => add(out, impl_owner(block), &f.attrs),
                    ImplItem::Const(c) => add(out, impl_owner(block), &c.attrs),
                    _ => {}
                }),
                Item::Trait(t) => t.items.iter().for_each(|inner| {
                    if let TraitItem::Fn(f) = inner {
                        add(out, Some(t.ident.to_string()), &f.attrs);
                    }
                }),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    add(&mut out, None, &file.attrs);
    walk(&file.items, &mut out);
    out
}

/// Prose as a claim is written: comments in it replaced by a space, whitespace
/// collapsed, trimmed.
fn normalise(prose: &str) -> String {
    let mut text = String::new();
    let mut rest = prose;
    while let Some(start) = rest.find("<!--") {
        text.push_str(&rest[..start]);
        text.push(' ');
        rest = rest[start..].find("-->").map_or("", |end| &rest[start + end + 3..]);
    }
    text.push_str(rest);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A fenced block of a doc comment: the line of its opening fence, its info string and
/// its code.
struct Fenced {
    line: usize,
    info: String,
    code: String,
}

/// Splits a doc comment into its prose and its fenced blocks.
fn split(comment: &DocComment) -> (String, Vec<Fenced>) {
    let (mut prose, mut blocks, mut open) = (String::new(), Vec::new(), None::<Fenced>);
    for (line, text) in &comment.lines {
        let fence = text.trim_start().strip_prefix("```");
        match (&mut open, fence) {
            (Some(_), Some(_)) => blocks.extend(open.take()),
            (Some(block), None) => block.code = format!("{}{text}\n", block.code),
            (None, Some(info)) => {
                open = Some(Fenced { line: *line, info: info.trim().to_string(), code: String::new() })
            }
            (None, None) => prose = format!("{prose}{text}\n"),
        }
    }
    (normalise(&prose), blocks)
}

fn parse(source: &SourceText) -> Option<syn::File> {
    is_source(&source.page).then(|| syn::parse_file(&source.text).ok()).flatten()
}

/// The doc comments of a source file as prose, in order.
pub fn prose(source: &SourceText) -> Vec<DocProse> {
    let Some(file) = parse(source) else { return Vec::new() };
    doc_comments(&file)
        .iter()
        .map(|comment| DocProse {
            text: split(comment).0,
            start: comment.lines[0].0,
            end: comment.lines[comment.lines.len() - 1].0,
        })
        .collect()
}

/// The words of an info string, when every one leaves the block a Rust example.
fn rust_words(info: &str) -> Option<Vec<&str>> {
    let words: Vec<&str> = info.split([',', ' ']).filter(|w| !w.is_empty()).collect();
    words.iter().all(|w| RUST_WORDS.contains(w)).then_some(words)
}

/// The examples the doc comments of a source file hold, as attestations. The claim an
/// example defends is the prose of its doc comment.
pub fn examples(source: &SourceText) -> Vec<Example> {
    let Some(file) = parse(source) else { return Vec::new() };
    let mut out = Vec::new();
    for comment in doc_comments(&file) {
        let (claim, blocks) = split(&comment);
        for block in blocks {
            let Some(words) = rust_words(&block.info) else { continue };
            let panics = words.contains(&SHOULD_PANIC);
            let runs = !words.contains(&"no_run");
            out.push(Example {
                id: format!("{}#L{}", source.page, block.line),
                page: source.page.clone(),
                line: block.line,
                claim: claim.clone(),
                kind: "example".into(),
                lang: "rust".into(),
                mode: if runs { "check" } else { "compile" }.into(),
                weak: runs && !panics && !text::asserts(&block.code),
                code: block.code,
                owner: comment.owner.clone(),
                tags: if panics { vec![SHOULD_PANIC.into()] } else { Vec::new() },
                ..Example::default()
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"//! A small crate.

/// Adds two numbers, <!-- a note --> whatever
/// their order.
///
/// ```
/// assert_eq!(kit::add(1, 2), 3);
/// ```
///
/// It never panics.
///
/// ```should_panic
/// kit::add(1, 2);
/// ```
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub struct Money;

impl Money {
    /// An amount.
    ///
    /// ```rust,no_run
    /// let m = kit::Money::new();
    /// ```
    ///
    /// ```sh
    /// cargo run
    /// ```
    ///
    /// ```ignore
    /// nothing
    /// ```
    pub fn new() -> Money {
        Money
    }
}
"#;

    fn source(page: &str) -> SourceText {
        SourceText { page: page.into(), text: SOURCE.into() }
    }

    #[test]
    fn examples_are_the_rust_blocks_of_doc_comments() {
        let found = examples(&source("src/lib.rs"));
        let read: Vec<_> = found
            .iter()
            .map(|e| (e.line, e.mode.as_str(), e.owner.as_deref(), e.weak, e.tags.len()))
            .collect();
        assert_eq!(
            read,
            [
                (6, "check", Some("add"), false, 0),
                (12, "check", Some("add"), false, 1),
                (24, "compile", Some("Money"), false, 0)
            ]
        );
        assert_eq!(found[0].claim, "Adds two numbers, whatever their order. It never panics.");
        assert_eq!(found[0].code, "assert_eq!(kit::add(1, 2), 3);\n");
        assert_eq!(found[0].id, "src/lib.rs#L6");
        assert!(examples(&source("docs/notes.txt")).is_empty());
    }

    #[test]
    fn prose_is_each_doc_comment_without_its_blocks() {
        let found = prose(&source("src/lib.rs"));
        let want = [
            ("A small crate.", 1, 1),
            ("Adds two numbers, whatever their order. It never panics.", 3, 14),
            ("An amount.", 22, 34),
        ];
        let read: Vec<_> = found.iter().map(|p| (p.text.as_str(), p.start, p.end)).collect();
        assert_eq!(read, want);
        assert!(prose(&source("docs/notes.txt")).is_empty());
    }
}
