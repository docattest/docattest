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
pub(crate) struct Case {
    pub(crate) name: String,
    pub(crate) line: usize,
}

/// The named cases of a test function, in the order its attributes give them.
pub(crate) fn named_cases(function: &ItemFn) -> Vec<Case> {
    let named = function.attrs.iter().filter_map(|attr| {
        let name = rstest_case(attr).or_else(|| test_case(attr))?;
        Some(Case { name, line: attr.span().start().line })
    });
    named.collect()
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

    #[test]
    fn values_name_no_case() {
        assert!(names("#[rstest]\nfn values(#[values(1, 2)] a: i32) {}").is_empty());
    }
}
