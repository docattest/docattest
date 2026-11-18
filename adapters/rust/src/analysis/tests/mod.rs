//! Tests, as a file declares them: read, never run.
//!
//! A test is a function with an attribute whose name ends in `test` (`#[test]`,
//! `#[tokio::test]`, `#[rstest]`) or is `test_case`, titled by its name, with the modules
//! written around it named first. A parametrized test's named cases are tests too, with
//! the function named around them ([`parametrized`]). A module is not
//! a test, and citing one says so.

mod parametrized;

use super::Analysis;
use crate::protocol::Declared;
use parametrized::named_cases;
use serde_json::{Value, json};
use syn::spanned::Spanned;
use syn::{Item, ItemFn};

/// A test function a file declares, with the modules written around it.
pub(crate) struct Test<'a> {
    pub(crate) declared: Declared,
    pub(crate) function: Option<&'a ItemFn>,
}

fn is_test(function: &ItemFn) -> bool {
    let named = |last: &syn::PathSegment| {
        let name = last.ident.to_string();
        name.ends_with("test") || name == "test_case"
    };
    function.attrs.iter().any(|attr| attr.path().segments.last().is_some_and(named))
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
                    let name = function.sig.ident.to_string();
                    let declared = entry(name.clone(), function.sig.span().start().line, None);
                    out.push(Test { declared, function: Some(function) });
                    let within = [around, &[name]].concat();
                    out.extend(named_cases(function).into_iter().map(|case| Test {
                        declared: Declared {
                            title: case.name,
                            describes: within.clone(),
                            line: case.line,
                            kind: None,
                        },
                        function: Some(function),
                    }));
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

#[cfg(test)]
mod listing {
    use super::*;

    /// What a file declares, as `describes > title` words, with the kind of what is no test.
    fn listed(source: &str) -> Vec<String> {
        let file = syn::parse_file(source).unwrap();
        let shown = |test: &Test| {
            let declared = &test.declared;
            let path = [declared.describes.clone(), vec![declared.title.clone()]].concat().join(" > ");
            declared.kind.as_ref().map_or(path.clone(), |kind| format!("{path} ({kind})"))
        };
        declared(&file).iter().map(shown).collect()
    }

    #[test]
    fn the_named_cases_of_parametrized_tests_are_tests() {
        let source = r#"
            mod tests {
                #[rstest]
                #[case::quarter(1000, 25, 750)]
                #[case(999, 50, 500)]
                fn discount(#[case] price: i64, #[case] percent: u32, #[case] result: i64) {}

                #[rstest]
                fn every_region(#[values("NL", "DE")] region: &str) {}

                #[test_case(1000, 25 => 750 ; "a quarter off")]
                #[test_case(999, 50 => 500)]
                fn rounds(price: i64, percent: u32) -> i64 { 0 }

                #[tokio::test]
                async fn waits() {}

                fn helper() {}
            }
        "#;
        assert_eq!(
            listed(source),
            [
                "tests (a module)",
                "tests > discount",
                "tests > discount > quarter",
                "tests > every_region",
                "tests > rounds",
                "tests > rounds > a quarter off",
                "tests > waits",
            ]
        );
    }
}
