//! Tests of planning a page: claims, placement and what each directive binds.

use super::*;
use crate::api::attestation::{FactTarget, SetFrom};

fn planned(text: &str) -> Vec<Attestation> {
    let languages = ["ts".to_string()];
    let options = ExampleOptions { languages: &languages, ignore_tags: &[], info_words: &[] };
    plan_page("p.md", text, &options).unwrap_or_else(|e| panic!("{e}")).attestations
}

fn claims(text: &str) -> Vec<(usize, String)> {
    planned(text).into_iter().map(|a| (a.line, a.claim)).collect()
}

/// What each fact on a page binds, and each citation's error.
fn outcomes(text: &str) -> Vec<Result<String, String>> {
    let outcome = |a: Attestation| match (a.body, a.error) {
        (_, Some(error)) => Err(error),
        (Body::Fact(fact), None) => fact.target.map(|t| format!("{t:?}")),
        (_, None) => Ok("cited".into()),
    };
    planned(text).into_iter().map(outcome).collect()
}

#[test]
fn a_directive_after_prose_claims_that_prose() {
    let page = "Plain prose.\n\nDoubling `4` gives `8`. <!-- fact: D -->\n\n## Port `80` <!-- fact: P -->";
    assert_eq!(claims(page), vec![(3, "Doubling `4` gives `8`.".into()), (5, "Port `80`".into())]);
}

#[test]
fn a_block_claims_the_paragraph_before_it_in_its_container() {
    let page = "Intro.\n\n- Item one.\n  <!-- test: t.ts > \"x\" -->\n- ```ts check\n  x\n  ```\n\n> Quoted.\n>\n> <!-- fact: K -->\n> - `a`";
    assert_eq!(claims(page), vec![(4, "Item one.".into()), (5, "Intro.".into()), (11, "Quoted.".into())]);
}

#[test]
fn a_fact_in_a_cell_claims_the_prose_before_its_table() {
    let table = "| a |\n|---|\n| `3` <!-- fact: N --> |";
    assert_eq!(claims(&format!("Limits:\n\n{table}")), vec![(5, "Limits:".into())]);
    assert_eq!(claims(table), vec![(3, "`3`".into())]);
}

#[test]
fn directives_alone_on_the_line_above_a_sentence_claim_it() {
    let page = "# Agent\n\n<!-- test: a.ts > \"x\" --> <!-- test: b.ts > \"y\" -->\nScripts run\nthe same way. Then more.";
    let claim = "Scripts run the same way.".to_string();
    assert_eq!(claims(page), vec![(3, claim.clone()), (3, claim)]);
    assert_eq!(outcomes(page), [Ok("cited".into()), Ok("cited".into())]);
}

#[test]
fn a_fact_alone_on_the_line_above_a_sentence_binds_as_at_its_end() {
    let scalar = |value: &str| Ok(format!("{:?}", FactTarget::Scalar(value.into())));
    let page = "<!-- fact: CURRENCY -->\nPrices are in `EUR` unless told otherwise. Not `USD`.";
    assert_eq!(outcomes(page), [scalar("EUR")]);
    assert_eq!(claims(page), [(1, "Prices are in `EUR` unless told otherwise.".into())]);
    let set = FactTarget::Set { items: vec!["a".into(), "b".into()], from: SetFrom::Sentence };
    assert_eq!(outcomes("<!-- fact: KEYS -->\nKeys are `a`\nand `b`.\n"), [Ok(format!("{set:?}"))]);
    let list = FactTarget::Set { items: vec!["a".into()], from: SetFrom::List };
    assert_eq!(outcomes("<!-- fact: KEYS -->\nThe keys:\n\n- `a`\n"), [Ok(format!("{list:?}"))]);
}

#[test]
fn a_directive_alone_then_a_blank_line_then_prose_is_an_error_there() {
    let page =
        "Intro.\n\n<!-- fact: CURRENCY -->\n\nPrices are in `EUR`.\n\n<!-- test: a.ts > \"x\" -->\n\nMore.";
    let error = |kind: &str, line: usize| Err(detached_error(kind, line));
    assert_eq!(outcomes(page), [error("fact", 3), error("test", 7)]);
}

/// A citation's line, file, alias, describes and title.
type Cited = (usize, String, Option<String>, Vec<String>, String);

/// Each citation on a page.
fn cited(text: &str) -> Vec<Cited> {
    let cite = |a: Attestation| match a.body {
        Body::Test(t) => Some((a.line, t.file, t.alias, t.describes, t.title)),
        _ => None,
    };
    planned(text).into_iter().filter_map(cite).collect()
}

#[test]
fn a_set_of_titles_is_a_citation_of_each() {
    let page = "Scripts run. <!-- test: agent/run_test.go > \"TestRun\" > {\"python\", \"node\"} -->";
    let run = |title: &str| {
        (1, "agent/run_test.go".to_string(), None, vec!["TestRun".to_string()], title.to_string())
    };
    assert_eq!(cited(page), [run("python"), run("node")]);
    assert_eq!(outcomes(page), [Ok("cited".into()), Ok("cited".into())]);
}

#[test]
fn a_citation_names_a_file_by_an_alias_declared_anywhere_on_its_page() {
    let page = "Python runs. <!-- test: launch > \"TestRun\" > \"python\" -->\n\n\
                <!-- tests: launch = ./agent/launch_test.go -->\n\nNode runs. <!-- test: launch > \"node\" -->";
    let file = "agent/launch_test.go".to_string();
    let alias = Some("launch".to_string());
    assert_eq!(
        cited(page),
        [
            (1, file.clone(), alias.clone(), vec!["TestRun".to_string()], "python".to_string()),
            (5, file, alias, vec![], "node".to_string()),
        ]
    );
    assert_eq!(cited("It runs. <!-- test: other > \"x\" -->")[0].1, "other");
}

#[test]
fn an_alias_declared_again_is_an_error_there_and_the_first_holds() {
    let page =
        "<!-- tests: a = a_test.go -->\n<!-- tests: a = b_test.go -->\n\nIt runs. <!-- test: a > \"x\" -->";
    assert_eq!(
        outcomes(page),
        [Err("tests at L2 declares a again: it is declared at L1".into()), Ok("cited".into())]
    );
    assert_eq!(cited(page)[1].1, "a_test.go");
}

#[test]
fn each_directive_says_where_it_took_effect_and_by_which_rule() {
    let languages = ["ts".to_string()];
    let options = ExampleOptions { languages: &languages, ignore_tags: &[], info_words: &[] };
    let page = "<!-- fact: N -->\nAt most `3` tries.\n\nIt runs. <!-- test: t.ts > \"x\" -->\n\n<!-- fact: K -->\n\nApart.\n\n```ts check\nx\n```";
    let parsed = parse_page(page).unwrap_or_default();
    let Explained { attestations, whys, .. } = plan_explained("p.md", &parsed, &options);
    let explained: Vec<Option<Why>> = attestations.iter().map(|a| whys.get(&a.id).copied()).collect();
    assert_eq!(
        explained,
        [
            Some(Why { placed: Placed::Above { line: 2 }, rule: Some(FactRule::CodeSpan) }),
            Some(Why { placed: Placed::Written, rule: None }),
            Some(Why { placed: Placed::Detached, rule: None }),
            None,
        ]
    );
}

#[test]
fn an_alias_in_a_table_that_cases_bind_is_the_tables_data() {
    let page = "<!-- cases: run -->\n| page |\n|---|\n| <!-- tests: a = a_test.go --> |\n| <!-- tests: a = b_test.go --> |\n";
    let kinds: Vec<_> = planned(page).into_iter().map(|a| (a.kind().as_str(), a.error)).collect();
    assert_eq!(kinds, [("cases", None)]);
}

#[test]
fn an_alias_left_unclosed_is_an_error_at_its_line() {
    let page = "# A\n\n<!-- tests: launch = a_test.go\n\nIt runs.";
    let error = "directive at L3 is not closed: end it with -->".to_string();
    assert_eq!(outcomes(page), [Err(error)]);
}
