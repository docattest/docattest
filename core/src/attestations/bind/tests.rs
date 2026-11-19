//! Tests of binding facts to their targets.

use super::*;
use crate::api::field::as_json_writes;
use crate::attestations::field::parse_field_path;
use crate::markdown::parse::parse_page;

fn directive(ordered: bool, column: Option<&str>, field: Option<&str>) -> FactDirective {
    FactDirective {
        expression: "E".into(),
        surface: "default".into(),
        ordered,
        column: column.map(Into::into),
        field: field.map(|written| parse_field_path(written).expect("a path")),
    }
}

fn bound(text: &str, ordered: bool) -> Result<FactTarget, String> {
    let page = parse_page(text).unwrap_or_default();
    let last = page.comments.last().expect("a comment");
    target(bind_fact(&directive(ordered, None, None), last.line, last, &page))
}

/// The target of a binding, without the rule.
fn target(bound: Result<Bound, String>) -> Result<FactTarget, String> {
    bound.map(|b| b.target)
}

fn sentence(items: &[&str]) -> Result<FactTarget, String> {
    Ok(FactTarget::Set { items: items.iter().map(|s| s.to_string()).collect(), from: SetFrom::Sentence })
}

#[test]
fn a_sentence_of_code_spans_binds_them_all() {
    assert_eq!(bound("Keys are `a`, `b` and `c`. <!-- f -->", false), sentence(&["a", "b", "c"]));
    assert_eq!(bound("Keys are `a`,\n`b` and `c`. <!-- f -->", true), sentence(&["a", "b", "c"]));
    assert_eq!(bound("See `x`. Keys are `a` and `b`. <!-- f -->", false), sentence(&["a", "b"]));
    assert_eq!(bound("See `x`.\nKeys are `a` and `b`. <!-- f -->", false), sentence(&["a", "b"]));
    assert_eq!(bound("`x` <!-- c --> `a` and `b`. <!-- f -->", false), sentence(&["a", "b"]));
    assert_eq!(bound("| k |\n|---|\n| `a` and `b` <!-- f --> |", false), sentence(&["a", "b"]));
    assert_eq!(bound("- `x`. Keys: `a`, `b`. <!-- f -->", false), sentence(&["a", "b"]));
    assert_eq!(bound("Keys are `a` and `b`\nhere. <!-- f -->", false), sentence(&["a", "b"]));
}

#[test]
fn a_single_value_stays_a_scalar() {
    assert_eq!(bound("Keys are `a`. <!-- f -->", false), Ok(FactTarget::Scalar("a".into())));
    assert_eq!(bound("`a` and `b` take 3 tries. <!-- f -->", false), Ok(FactTarget::Scalar("3".into())));
    assert_eq!(bound("`a` and `b`. Then more. <!-- f -->", false), Ok(FactTarget::Scalar("b".into())));
    assert_eq!(
        bound("Released as 1.2.3-rc.1. <!-- f -->", false),
        Ok(FactTarget::Scalar("1.2.3-rc.1".into()))
    );
    assert!(bound("Keys are `a`. <!-- f -->", true).is_err());
    assert_eq!(bound("The key is `a`\nhere. <!-- f -->", false), Ok(FactTarget::Scalar("a".into())));
    assert!(bound("See `x`. No value\nhere. <!-- f -->", false).is_err());
}

#[test]
fn a_directive_ending_a_paragraph_with_no_target_binds_the_list_after_it() {
    let list = |items: &[&str]| {
        Ok(FactTarget::Set { items: items.iter().map(|s| s.to_string()).collect(), from: SetFrom::List })
    };
    assert_eq!(bound("Three keys: <!-- f -->\n\n- `a`\n- `b`", false), list(&["a", "b"]));
    assert_eq!(bound("<!-- f -->\n\n\n- `a`", false), list(&["a"]));
    assert_eq!(bound("Shape: <!-- f -->\n\n```ts\nx\n```", false), Ok(FactTarget::Text("x".into())));
    assert_eq!(bound("Keys are `a`. <!-- f -->\n\n- `b`", false), Ok(FactTarget::Scalar("a".into())));
    assert!(bound("Three keys: <!-- f --> more\n\n- `a`", false).is_err());
    assert!(bound("Three keys: <!-- f -->\n\nProse.\n\n- `a`", false).is_err());
    assert!(
        bound("Three keys: <!-- f -->\n\n| a |\n|---|\n| b |", false)
            .unwrap_err()
            .contains("names no column")
    );
}

#[test]
fn directives_side_by_side_bind_alike_in_either_order() {
    let set = |items: &[&str], from: SetFrom| {
        Ok(FactTarget::Set { items: items.iter().map(|s| s.to_string()).collect(), from })
    };
    let cite = "<!-- test: t > \"x\" -->";
    let pages = [
        ("Tax in `NL` and: {group}\n- `NL`\n- `DE`", set(&["NL", "DE"], SetFrom::List)),
        ("{group}\n- `a`\n- `b`", set(&["a", "b"], SetFrom::List)),
        ("{stack}\n- `a`\n- `b`", set(&["a", "b"], SetFrom::List)),
        ("Three keys: {group}\n\n- `a`", set(&["a"], SetFrom::List)),
        ("Three keys: {stack}\n\n- `a`", set(&["a"], SetFrom::List)),
        ("Keys are `a` and `b`. {group}", set(&["a", "b"], SetFrom::Sentence)),
        ("| k |\n|---|\n| `a` and `b` {group} |", set(&["a", "b"], SetFrom::Sentence)),
        ("At most `3` tries. {group}", Ok(FactTarget::Scalar("3".into()))),
    ];
    for (page, expected) in pages {
        for (first, second) in [("<!-- f -->", cite), (cite, "<!-- f -->")] {
            let text = page
                .replace("{group}", &format!("{first} {second}"))
                .replace("{stack}", &format!("{first}\n{second}"));
            let parsed = parse_page(&text).unwrap_or_default();
            let fact = parsed.comments.iter().find(|c| trim(&c.body) == "f").expect("the fact");
            let got = target(bind_fact(&directive(false, None, None), fact.line, fact, &parsed));
            assert_eq!(got, expected, "{text}");
        }
    }
}

#[test]
fn column_facts_stacked_above_one_table_each_bind_it() {
    let column = |items: &[&str], name: &str| {
        Ok(FactTarget::Set {
            items: items.iter().map(|s| s.to_string()).collect(),
            from: SetFrom::Table { column: name.into() },
        })
    };
    let table = "| method | path |\n|---|---|\n| GET | `/a` |";
    for gap in ["\n", "\n\n"] {
        let page = parse_page(&format!("<!-- f -->{gap}<!-- g -->\n{table}")).unwrap_or_default();
        let [first, second] = &page.comments[..] else { panic!("two comments") };
        assert_eq!(
            target(bind_fact(&directive(false, Some("method"), None), first.line, first, &page)),
            column(&["GET"], "method")
        );
        assert_eq!(
            target(bind_fact(&directive(false, Some("path"), None), second.line, second, &page)),
            column(&["/a"], "path")
        );
    }
}

#[test]
fn field_facts_stacked_above_one_block_each_bind_it() {
    let block = "```json\n{\"port\": 2222, \"hosts\": [\"a\", \"b\"]}\n```";
    for gap in ["\n", "\n\n"] {
        let page = parse_page(&format!("The job: <!-- f -->{gap}<!-- g -->\n{block}")).unwrap_or_default();
        let [first, second] = &page.comments[..] else { panic!("two comments") };
        let spelled = |bound: Result<Bound, String>| target(bound).map(|t| t.spelled(as_json_writes));
        let port = spelled(bind_fact(&directive(false, None, Some("port")), first.line, first, &page));
        assert_eq!(port, Ok(FactTarget::Scalar("2222".into())));
        let hosts = spelled(bind_fact(&directive(true, None, Some("hosts")), second.line, second, &page));
        let items = vec!["a".to_string(), "b".to_string()];
        assert_eq!(hosts, Ok(FactTarget::Set { items, from: SetFrom::Field { path: "hosts".into() } }));
    }
    let page = parse_page(&format!("Port `1` <!-- f --> here.\n{block}")).unwrap_or_default();
    let inline = target(bind_fact(&directive(false, None, Some("port")), 1, &page.comments[0], &page));
    assert_eq!(inline, Err("fact at L1 has field=port but no fenced block follows it".into()));
}

#[test]
fn each_binding_says_the_rule_that_made_it() {
    let rule = |text: &str, column: Option<&str>, field: Option<&str>| {
        let page = parse_page(text).unwrap_or_default();
        let last = page.comments.last().expect("a comment");
        bind_fact(&directive(false, column, field), last.line, last, &page).map(|b| b.rule)
    };
    let fact = |text: &str| rule(text, None, None);
    assert_eq!(rule("<!-- f -->\n```json\n{\"a\": 1}\n```", None, Some("a")), Ok(FactRule::Field));
    assert_eq!(rule("<!-- f -->\n| k |\n|---|\n| a |", Some("k"), None), Ok(FactRule::Column));
    assert_eq!(fact("Keys: <!-- f -->\n- `a`"), Ok(FactRule::BlockBelow));
    assert_eq!(fact("At most `3` tries. <!-- f -->"), Ok(FactRule::CodeSpan));
    assert_eq!(fact("At most 3 tries. <!-- f -->"), Ok(FactRule::Literal));
    assert_eq!(fact("Keys are `a` and `b`. <!-- f -->"), Ok(FactRule::SentenceSpans));
    assert_eq!(fact("The key is `a`\nhere. <!-- f -->"), Ok(FactRule::SentenceSpan));
    assert_eq!(fact("Keys: <!-- f -->\n\n- `a`"), Ok(FactRule::BlockAfter));
}
