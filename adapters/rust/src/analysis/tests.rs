//! Tests, as a file declares them: read, never run.
//!
//! A test is a function with a `#[test]` attribute (or one that ends in `test`, such as
//! `#[tokio::test]`), titled by its name, with the modules written around it named
//! first. A module is not a test, and citing one says so.

use super::Analysis;
use crate::protocol::Declared;
use serde_json::{Value, json};
use syn::spanned::Spanned;
use syn::{Item, ItemFn};

/// A test function a file declares, with the modules written around it.
pub(crate) struct Test<'a> {
    pub(crate) declared: Declared,
    pub(crate) function: Option<&'a ItemFn>,
}

fn is_test(function: &ItemFn) -> bool {
    function.attrs.iter().any(|attr| attr.path().segments.last().is_some_and(|last| last.ident == "test"))
}

/// What a file declares that can be cited, or cited by mistake: its tests, and the
/// modules that hold them.
pub(crate) fn declared(file: &syn::File) -> Vec<Test<'_>> {
    fn walk<'a>(items: &'a [Item], around: &[String], out: &mut Vec<Test<'a>>) {
        for item in items {
            let entry = |title: String, line: usize, kind: Option<&str>| Declared {
                title,
                describes: around.to_vec(),
                line,
                kind: kind.map(str::to_string),
            };
            match item {
                Item::Fn(function) if is_test(function) => {
                    let declared =
                        entry(function.sig.ident.to_string(), function.sig.span().start().line, None);
                    out.push(Test { declared, function: Some(function) });
                }
                Item::Mod(module) => {
                    let Some((_, inner)) = &module.content else { continue };
                    let name = module.ident.to_string();
                    out.push(Test {
                        declared: entry(name.clone(), module.span().start().line, Some("a module")),
                        function: None,
                    });
                    walk(inner, &[around, &[name]].concat(), out);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&file.items, &[], &mut out);
    out
}

/// The test a citation names: the title, and the enclosing names when it gives any.
pub(crate) fn cited<'a, 'b>(all: &'b [Test<'a>], title: &str, describes: &[String]) -> Option<&'b Test<'a>> {
    all.iter().find(|test| {
        let declared = &test.declared;
        declared.kind.is_none()
            && declared.title == title
            && (describes.is_empty() || declared.describes == describes)
    })
}

impl Analysis {
    /// Answers `tests`.
    pub(crate) fn list_tests(&self, file: &str) -> Value {
        let Ok(text) = std::fs::read_to_string(self.abs(file)) else {
            return json!({ "ok": false, "message": format!("{file} does not exist") });
        };
        let parsed = match syn::parse_file(&text) {
            Ok(parsed) => parsed,
            Err(err) => return json!({ "ok": false, "message": format!("{file} does not parse: {err}") }),
        };
        let all = declared(&parsed);
        let (suites, tests): (Vec<_>, Vec<_>) =
            all.iter().map(|test| &test.declared).partition(|d| d.kind.is_some());
        json!({ "ok": true, "tests": tests, "suites": suites })
    }
}
