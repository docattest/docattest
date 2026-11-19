//! Tests of what `explain` says, over pages planned in memory and results made by hand.

use serde_json::json;

use super::*;
use crate::api::attestation::Kind;
use crate::attestations::examples::ExampleOptions;
use crate::attestations::plan::{Explained, plan_explained};
use crate::markdown::parse::parse_page;
use crate::report::Status;

/// Each attestation of a page explained, with one result of `status` and value `actual`.
fn explained(text: &str, status: Status, actual: &str) -> Vec<Explanation> {
    let options = ExampleOptions { languages: &[], ignore_tags: &[], info_words: &[] };
    let Explained { attestations, whys, aliases } =
        plan_explained("p.md", &parse_page(text).unwrap(), &options);
    let page: Vec<&Attestation> = attestations.iter().collect();
    let named = aliases.declared().iter().map(|(alias, line)| Explanation::of_alias(alias, *line, &page));
    let attesting = attestations.iter().map(|a| {
        let result = Outcome::new("p.md", a.line, a.kind(), status, "said").with_comparison("x", actual);
        Explanation::Attests { attestation: Box::new(a.clone()), why: whys[&a.id], results: vec![result] }
    });
    named.chain(attesting).collect()
}

#[test]
fn a_fact_says_where_it_took_effect_what_it_bound_by_which_rule_and_what_its_value_is() {
    let text = format(
        "p.md",
        &explained("<!-- fact @api ordered: KEYS -->\nKeys are `a`\nand `b`.", Status::Fail, "[b a]"),
    );
    assert_eq!(
        text,
        [
            "p.md:1 fact: KEYS",
            "  claim     Keys are `a` and `b`.",
            "  placed    on the sentence below it, as if written where that sentence ends, on L3",
            "  binds     the code spans of the sentence it ends, as a set",
            "  target    a, b (2 items)",
            "  surface   api, in order",
            "  value     [b a]",
            "  result    ✗ fail: said",
        ]
        .join("\n")
    );
}

#[test]
fn a_block_target_is_shown_line_by_line_and_a_detached_fact_binds_nothing() {
    let block =
        format("p.md", &explained("Shape: <!-- fact: S -->\n\n```ts\na\nb\n```", Status::Pass, "a b"));
    assert!(
        block.contains(
            "  binds     the list or fenced block after the paragraph it ends\n  target    a\n            b\n"
        ),
        "{block}"
    );
    let apart = format("p.md", &explained("<!-- fact: N -->\n\nProse `3`.", Status::Error, ""));
    assert!(
        apart.contains(
            "  placed    nowhere: a blank line parts it from the prose below\n  surface   default\n"
        ),
        "{apart}"
    );
}

#[test]
fn a_citation_by_an_alias_names_the_file_and_the_alias_names_its_citations() {
    let page =
        "<!-- tests: run = t/run_test.go -->\n\nIt runs. <!-- test: run > \"TestRun\" > {\"a\", \"b\"} -->";
    let explanations = explained(page, Status::Pass, "");
    let text = format("p.md", &explanations);
    assert!(text.starts_with("p.md:1 tests: run = t/run_test.go\n  cited by  L3\n\np.md:3 test: t/run_test.go > \"TestRun\" > \"a\"\n"), "{text}");
    assert!(
        text.contains(
            "  file      t/run_test.go, by the alias run\n  result    ✓ pass: said\n\np.md:3 test:"
        ),
        "{text}"
    );
    let json = to_json("p.md", &explanations);
    assert_eq!(
        json["directives"][0],
        json!({ "line": 1, "kind": "alias", "name": "run", "file": "t/run_test.go", "citedAt": [3] })
    );
    assert_eq!(json["directives"][1]["alias"], json!("run"));
    assert_eq!(json["directives"][1]["attestation"]["title"], json!("a"));
}

#[test]
fn the_json_form_names_the_placement_and_the_rule() {
    let json = to_json("p.md", &explained("<!-- fact: N -->\nAt most 3 tries.", Status::Pass, "3"));
    let fact = &json["directives"][0];
    assert_eq!(fact["placed"], json!({ "where": "above", "line": 2 }));
    assert_eq!(fact["rule"], json!("literal"));
    assert_eq!(fact["attestation"]["target"], json!({ "kind": "scalar", "value": "3" }));
    assert_eq!(fact["results"][0]["actual"], json!("3"));
    assert_eq!(fact["kind"], json!(Kind::Fact.as_str()));
}

#[test]
fn a_cases_table_shows_its_rows_and_each_rows_result_by_line() {
    let page = "<!-- cases: sum -->\n| a | sum |\n|---|---|\n| 1 | 1 |";
    let mut explanations = explained(page, Status::Pass, "");
    let Explanation::Attests { results, .. } = &mut explanations[0] else { panic!("cases") };
    results[0].line = 4;
    let text = format("p.md", &explanations);
    assert!(
        text.ends_with("  binds     the table after it: 1 row, columns a, sum\n  result    ✓ L4 pass: said"),
        "{text}"
    );
}

#[test]
fn a_malformed_directive_is_named_by_its_kind_and_its_error_is_the_result() {
    let text = format("p.md", &explained("It runs. <!-- test: t.ts -->", Status::Error, ""));
    assert_eq!(
        text,
        "p.md:1 test\n  claim     It runs.\n  placed    where it is written\n  result    ! error: said"
    );
}
