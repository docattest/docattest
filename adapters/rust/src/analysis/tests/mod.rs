//! Tests, as a file declares them: read, never run.
//!
//! A test is a function with an attribute whose name ends in `test` (`#[test]`,
//! `#[tokio::test]`, `#[rstest]`) or is `test_case`, titled by its name, with the modules
//! written around it named first. A parametrized test's named cases are tests too, with
//! the function named around them ([`parametrized`]). A module is not
//! a test, and citing one says so. What `cargo test` prints for a test is read from
//! syntax too ([`Printed`]), for the run of the tests that citations name.

mod parametrized;

use super::Analysis;
use crate::protocol::Declared;
use parametrized::{Case, is_parametrized, named_cases};
use serde_json::{Value, json};
use syn::spanned::Spanned;
use syn::{Item, ItemFn};

/// A test function a file declares, with the modules written around it.
pub(crate) struct Test<'a> {
    pub(crate) declared: Declared,
    pub(crate) function: Option<&'a ItemFn>,
    /// The case it is, when it is a named case of a parametrized function.
    case: Option<Case>,
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
                    out.push(Test { declared, function: Some(function), case: None });
                    let within = [around, &[name]].concat();
                    out.extend(named_cases(function).into_iter().map(|case| Test {
                        declared: Declared {
                            title: case.name.clone(),
                            describes: within.clone(),
                            line: case.line,
                            kind: None,
                        },
                        function: Some(function),
                        case: Some(case),
                    }));
                }
                Item::Mod(module) => {
                    let Some((_, inner)) = &module.content else { continue };
                    let name = module.ident.to_string();
                    out.push(Test {
                        declared: entry(name.clone(), module.span().start().line, Some("a module")),
                        function: None,
                        case: None,
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

/// What `cargo test` runs of a declared test, by the paths it prints for its tests below
/// the module of the test's file.
#[derive(Clone, Debug)]
pub(crate) enum Printed {
    /// A test function, printed by its own path.
    Function(Vec<String>),
    /// A function a macro made a module of cases: every test below its path.
    Cases(Vec<String>),
    /// A named case of such a function: the test below the function's path that the macro
    /// named after the case.
    Case { function: Vec<String>, case: Case },
}

impl Printed {
    /// Whether a path `cargo test` prints, below the module of the test's file, is one of
    /// this test's.
    pub(crate) fn includes(&self, printed: &[&str]) -> bool {
        match self {
            Printed::Function(path) => printed.len() == path.len() && begins_with(printed, path),
            Printed::Cases(path) => begins_with(printed, path),
            Printed::Case { function, case } => {
                begins_with(printed, function)
                    && printed.get(function.len()).is_some_and(|name| case.printed_as(name))
            }
        }
    }
}

fn begins_with(printed: &[&str], path: &[String]) -> bool {
    printed.len() >= path.len() && path.iter().zip(printed).all(|(written, shown)| written == shown)
}

/// How `cargo test` prints the test a file declares at exactly this path.
pub(crate) fn printed(file: &syn::File, describes: &[String], title: &str) -> Option<Printed> {
    let all = declared(file);
    let test = all.iter().find(|test| {
        let declared = &test.declared;
        declared.kind.is_none() && declared.title == title && declared.describes == describes
    })?;
    let path = [describes, &[title.to_string()]].concat();
    Some(match (&test.case, test.function) {
        (Some(case), _) => Printed::Case { function: describes.to_vec(), case: case.clone() },
        (None, Some(function)) if is_parametrized(function) => Printed::Cases(path),
        _ => Printed::Function(path),
    })
}

impl Analysis {
    /// How `cargo test` prints a test a project file declares, or why it cannot say.
    pub(crate) fn printed(&self, file: &str, describes: &[String], title: &str) -> Result<Printed, String> {
        let text = std::fs::read_to_string(self.abs(file)).map_err(|_| format!("{file} does not exist"))?;
        let parsed = syn::parse_file(&text).map_err(|err| format!("{file} does not parse: {err}"))?;
        let path = [describes, &[title.to_string()]].concat().join(" > ");
        printed(&parsed, describes, title).ok_or_else(|| format!("{file} declares no test {path}"))
    }

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

    /// Which of some paths `cargo test` prints are of the test a source declares at a path.
    fn included<'a>(source: &str, path: &[&str], printed: &[&'a str]) -> Vec<&'a str> {
        let file = syn::parse_file(source).unwrap();
        let (title, describes) = path.split_last().unwrap();
        let describes: Vec<String> = describes.iter().map(|name| name.to_string()).collect();
        let test = printed_test(&file, &describes, title);
        printed.iter().filter(|name| test.includes(&name.split("::").collect::<Vec<_>>())).copied().collect()
    }

    fn printed_test(file: &syn::File, describes: &[String], title: &str) -> Printed {
        printed(file, describes, title).unwrap_or_else(|| panic!("no test {title}"))
    }

    #[test]
    fn a_test_is_printed_by_its_path_and_a_parametrized_one_by_its_cases() {
        let source = r#"
            #[test]
            fn plain() {}
            mod plain { #[test] fn inner() {} }
            mod tests {
                #[rstest]
                #[case(1)]
                #[case::quarter(25)]
                fn discount(#[case] percent: u32) {}

                #[test_case(25 ; "a quarter off")]
                fn rounds(percent: u32) {}
            }
        "#;
        let listed = [
            "plain",
            "plain::inner",
            "tests::discount::case_1",
            "tests::discount::case_2_quarter",
            "tests::discountless",
            "tests::rounds::test_case_1_a_quarter_off",
        ];
        assert_eq!(included(source, &["plain"], &listed), ["plain"]);
        assert_eq!(
            included(source, &["tests", "discount"], &listed),
            ["tests::discount::case_1", "tests::discount::case_2_quarter"]
        );
        assert_eq!(
            included(source, &["tests", "discount", "quarter"], &listed),
            ["tests::discount::case_2_quarter"]
        );
        assert_eq!(
            included(source, &["tests", "rounds", "a quarter off"], &listed),
            ["tests::rounds::test_case_1_a_quarter_off"]
        );
        let file = syn::parse_file(source).unwrap();
        assert!(printed(&file, &[], "discount").is_none());
        assert!(printed(&file, &[], "inner").is_none());
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
