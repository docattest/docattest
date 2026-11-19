//! The cases of a parametrized test, by the names its source writes for them.
//!
//! `cargo test` runs each case of an rstest or test-case test as a test of its own, in a
//! module named after the function (`tests::discount::case_1_big`). The names it prints
//! begin with the case's position (`case_1_`, and `test_case_1_` from test-case 3.4 on),
//! which a case added above shifts, so a case is cited by what its source writes instead:
//! the name after `case::` in rstest's `#[case::big(…)]`, and the string after the `;` in
//! test-case's `#[test_case(… ; "big")]`. A case its source gives no name (`#[case(…)]`,
//! `#[test_case(…)]`, an argument of `#[values(…)]`) is named by its position or its
//! values only, and cannot be cited.

use proc_macro2::{TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::{Attribute, ItemFn, Lit, Meta};

/// A case of a parametrized test that its source names.
#[derive(Clone, Debug)]
pub(crate) struct Case {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) by: CaseMacro,
}

/// The macro that writes a test's cases, which decides the names `cargo test` prints for them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CaseMacro {
    Rstest,
    TestCase,
}

/// The named cases of a test function, in the order its attributes give them.
pub(crate) fn named_cases(function: &ItemFn) -> Vec<Case> {
    let named = function.attrs.iter().filter_map(|attr| {
        let (name, by) = rstest_case(attr)
            .map(|name| (name, CaseMacro::Rstest))
            .or_else(|| test_case(attr).map(|name| (name, CaseMacro::TestCase)))?;
        Some(Case { name, line: attr.span().start().line, by })
    });
    named.collect()
}

/// Whether a macro makes a test function a module of cases, each a test of its own.
pub(crate) fn is_parametrized(function: &ItemFn) -> bool {
    let named = |last: &syn::PathSegment| last.ident == "rstest" || last.ident == "test_case";
    function.attrs.iter().any(|attr| attr.path().segments.last().is_some_and(named))
}

impl Case {
    /// Whether a name `cargo test` prints in the module of the case's function is this
    /// case's. The position it begins with is any: either way the source's name decides.
    pub(crate) fn printed_as(&self, printed: &str) -> bool {
        match self.by {
            CaseMacro::Rstest => after_position(printed, "case_") == Some(format!("_{}", self.name).as_str()),
            CaseMacro::TestCase => {
                let escaped = identifier(&self.name);
                let numbered = if escaped.starts_with('_') { escaped.clone() } else { format!("_{escaped}") };
                printed == escaped || after_position(printed, "test_case_") == Some(numbered.as_str())
            }
        }
    }
}

/// What follows a prefix and the position after it: `_quarter` of `case_02_quarter`.
fn after_position<'a>(printed: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = printed.strip_prefix(prefix)?;
    let position = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    (position > 0).then(|| &rest[position..])
}

/// The identifier test-case writes for a description, as its `escape_test_name` does:
/// lower case, each run of anything but letters and digits one `_`, and a `_` first when
/// the first character cannot begin an identifier.
fn identifier(description: &str) -> String {
    if description.is_empty() {
        return "_empty".to_string();
    }
    let mut written = String::new();
    for c in description.to_ascii_lowercase().chars() {
        if c.is_alphanumeric() {
            written.push(c);
        } else if !written.ends_with('_') {
            written.push('_');
        }
    }
    if written.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic()) {
        written
    } else {
        format!("_{written}")
    }
}

/// The name of an rstest case, `#[case::name(…)]`.
fn rstest_case(attr: &Attribute) -> Option<String> {
    let segments: Vec<String> =
        attr.path().segments.iter().map(|segment| segment.ident.to_string()).collect();
    match segments.as_slice() {
        [case, name] if case == "case" => Some(name.clone()),
        _ => None,
    }
}

/// The description of a test-case case, the string literal after the last `;` of
/// `#[test_case(… ; "description")]`.
fn test_case(attr: &Attribute) -> Option<String> {
    if !attr.path().segments.last().is_some_and(|last| last.ident == "test_case") {
        return None;
    }
    let Meta::List(list) = &attr.meta else { return None };
    let after = after_last_semicolon(list.tokens.clone())?;
    match syn::parse2::<Lit>(after).ok()? {
        Lit::Str(description) => Some(description.value()),
        _ => None,
    }
}

/// What follows the last `;` written at the top level of some tokens.
fn after_last_semicolon(tokens: TokenStream) -> Option<TokenStream> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let at = trees.iter().rposition(|tree| matches!(tree, TokenTree::Punct(p) if p.as_char() == ';'))?;
    Some(trees[at + 1..].iter().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(function: &str) -> Vec<(String, usize)> {
        let parsed: ItemFn = syn::parse_str(function).unwrap();
        named_cases(&parsed).into_iter().map(|case| (case.name, case.line)).collect()
    }

    #[test]
    fn rstest_cases_are_named_after_case() {
        let function = "#[rstest]\n#[case::small(1, 2)]\n#[case(2, 2)]\n#[case::big_one(100, 1)]\nfn adds(#[case] a: i32, #[case] b: i32) {}";
        assert_eq!(names(function), [("small".to_string(), 2), ("big_one".to_string(), 4)]);
    }

    #[test]
    fn test_case_cases_are_named_by_their_description() {
        let function = "#[test_case(1, 2 ; \"one and two\")]\n#[test_case(3, 3)]\n#[test_case::test_case(2 => 4 ; \"Two & Two!\")]\n#[test_case(\"a;b\")]\nfn sums(a: i32) {}";
        assert_eq!(names(function), [("one and two".to_string(), 1), ("Two & Two!".to_string(), 3)]);
    }

    fn case(name: &str, by: CaseMacro) -> Case {
        Case { name: name.to_string(), line: 1, by }
    }

    #[test]
    fn an_rstest_case_is_printed_after_its_position() {
        let quarter = case("quarter", CaseMacro::Rstest);
        assert!(quarter.printed_as("case_2_quarter"));
        assert!(quarter.printed_as("case_02_quarter"));
        for other in ["case_2", "case__quarter", "case_2_quarter_off", "quarter", "test_case_2_quarter"] {
            assert!(!quarter.printed_as(other), "{other}");
        }
    }

    #[test]
    fn a_test_case_case_is_printed_as_its_description_made_an_identifier() {
        let quarter = case("A quarter, off!", CaseMacro::TestCase);
        assert!(quarter.printed_as("a_quarter_off_"));
        assert!(quarter.printed_as("test_case_3_a_quarter_off_"));
        for other in ["a_quarter_off", "test_case__a_quarter_off_", "case_3_a_quarter_off_"] {
            assert!(!quarter.printed_as(other), "{other}");
        }
        let digit = case("1 cent", CaseMacro::TestCase);
        assert!(digit.printed_as("_1_cent") && digit.printed_as("test_case_1_1_cent"));
        assert!(case("", CaseMacro::TestCase).printed_as("test_case_1_empty"));
    }

    #[test]
    fn rstest_and_test_case_make_a_test_a_module_of_cases() {
        let parametrized = |function: &str| is_parametrized(&syn::parse_str(function).unwrap());
        assert!(parametrized("#[rstest]\nfn a() {}"));
        assert!(parametrized("#[test_case::test_case(1)]\nfn a(n: u8) {}"));
        assert!(!parametrized("#[test]\nfn a() {}"));
    }

    #[test]
    fn values_name_no_case() {
        assert!(names("#[rstest]\nfn values(#[values(1, 2)] a: i32) {}").is_empty());
    }
}
