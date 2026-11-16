//! Checks of a session's methods: compile and run, evaluate, and what the index and the
//! report need.

use serde_json::{Value, json};

use super::answer::{items, sorted_keys};
use crate::conformance::expect::{Checked, all_hold, ensure, same};
use crate::conformance::kit::{Facts, Kit};
use crate::conformance::requests::{MANIFEST, MANIFEST_VERSION};
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::Conversation;

pub const CHECKS: [Check; 3] = [
    Check {
        name: "compile, run: a verdict for each example, at its own page and line",
        run: compiles_and_runs,
    },
    Check {
        name: "evaluate: every fact has an answer; a value is displayed as the language displays it",
        run: evaluates,
    },
    Check {
        name: "symbols, exports, describeSurfaces, inCode: what the index and the report need",
        run: indexes,
    },
];

fn compiles_and_runs(suite: &Suite) -> Checked<Conversation> {
    let (requests, examples) = (&suite.requests, &suite.kit.examples);
    let passes = requests.example("passes", &examples.passes);
    let fails = requests.example("fails", &examples.fails);
    let broken = requests.example_in_mode("broken", &examples.broken, "compile");
    let all = json!([passes, fails, broken]);
    let run = |examples: Value| ("run", json!({ "examples": examples, "cases": [] }));
    let t = suite.session(
        &[
            ("compile", json!({ "examples": all })),
            run(json!([passes])),
            run(json!([fails])),
            run(json!([passes, fails])),
            run(json!([])),
            ("close", json!({})),
        ],
        all.clone(),
    )?;
    all_hold([
        t.answered(2, &json!({}), "open"),
        t.result(3).and_then(only_the_broken_is_diagnosed),
        t.result(4).and_then(|results| one_verdict(results, &passes, "pass")),
        t.result(5).and_then(|results| one_verdict(results, &fails, "fail")),
        t.result(6).and_then(|results| in_order(results, &passes, &fails)),
        t.answered(7, &json!([]), "a run of nothing"),
        t.answered(8, &json!({}), "close"),
        t.exited_cleanly(),
    ])?;
    Ok(t)
}

/// Diagnostics are keyed by example id, and only what does not compile has any.
fn only_the_broken_is_diagnosed(diagnostics: &Value) -> Checked {
    let mut diagnosed: Vec<&Value> = Vec::new();
    for diagnostic in items(diagnostics) {
        if !diagnosed.contains(&&diagnostic["id"]) {
            diagnosed.push(&diagnostic["id"]);
        }
    }
    let said = "diagnostics are keyed by example id, and only what does not compile has any";
    same(&json!(diagnosed), &json!(["broken"]), said)
}

/// Results come in the order of the examples.
fn in_order(results: &Value, passes: &Value, fails: &Value) -> Checked {
    let verdicts: Vec<Value> = items(results).iter().map(|r| json!([r["line"], r["status"]])).collect();
    let expected = json!([[passes["line"], "pass"], [fails["line"], "fail"]]);
    same(&json!(verdicts), &expected, "results come in the order of the examples")
}

/// One example run, one result: at the example's page and line, with `status` and a message.
fn one_verdict(results: &Value, example: &Value, status: &str) -> Checked {
    let [result] = items(results) else {
        return Err(format!("one example run, one result: got {results}"));
    };
    let verdict = json!([result["page"], result["line"], result["kind"], result["status"]]);
    same(&verdict, &json!([example["page"], example["line"], "example", status]), "the verdict")?;
    ensure(result["message"].as_str().is_some_and(|m| !m.is_empty()), "a result has a message")
}

fn evaluates(suite: &Suite) -> Checked<Conversation> {
    let (requests, facts) = (&suite.requests, &suite.kit.facts);
    let asked = json!([
        requests.fact("scalar", &facts.scalar.expression),
        requests.fact("list", &facts.list.expression),
        requests.fact("text", &facts.text.expression),
        requests.fact("invalid", &facts.invalid),
        requests.fact("manifest", &facts.manifest),
        requests.fact("unread", &facts.manifest.replace(MANIFEST, UNREAD)),
    ]);
    let t = suite.session(
        &[("evaluate", json!({ "facts": asked })), ("evaluate", json!({ "facts": [] }))],
        json!([]),
    )?;
    all_hold([
        t.result(3).and_then(|values| expect_values(values, facts)),
        t.answered(4, &json!({}), "evaluate of no facts"),
    ])?;
    Ok(t)
}

/// A manifest the session was not opened with.
const UNREAD: &str = "unread.json";

/// A value for every fact, each displayed as the kit says; none for the invalid one, nor for
/// a manifest the session does not have.
fn expect_values(values: &Value, facts: &Facts) -> Checked {
    let (scalar, text, invalid) = (&values["scalar"]["value"], &values["text"]["value"], &values["invalid"]);
    let (manifest, unread) = (&values["manifest"]["value"], &values["unread"]);
    all_hold([
        same(
            &json!(sorted_keys(values)),
            &json!(["invalid", "list", "manifest", "scalar", "text", "unread"]),
            "a value for every fact",
        ),
        same(
            &json!([manifest["text"], manifest["kind"]]),
            &json!([MANIFEST_VERSION, "text"]),
            "a manifest's field",
        ),
        same(&unread["ok"], &json!(false), "a manifest the session was not opened with"),
        ensure(
            unread["message"].as_str().is_some_and(|m| m.contains(UNREAD)),
            "a manifest that cannot be read is named in the message",
        ),
        same(&scalar["text"], &json!(facts.scalar.text), "the scalar's text"),
        ensure(scalar.get("kind").is_none(), "only text says what kind it is"),
        same(&values["list"]["value"]["items"], &json!(facts.list.items), "the list's items"),
        same(
            &json!([text["text"], text["kind"], text.get("items")]),
            &json!([facts.text.text, "text", null]),
            "text is text, and has no items",
        ),
        same(&invalid["ok"], &json!(false), "an expression that does not evaluate"),
        ensure(invalid["message"].as_str().is_some_and(|m| !m.is_empty()), "a failed evaluation says why"),
    ])
}

fn indexes(suite: &Suite) -> Checked<Conversation> {
    let (requests, kit) = (&suite.requests, suite.kit);
    let mut wrong = requests.fact("wrong", &kit.facts.scalar.expression);
    wrong["error"] = json!("planned with an error");
    let attestations = json!([
        requests.example("e", &kit.examples.passes),
        requests.fact("f", &kit.facts.scalar.expression),
        wrong,
    ]);
    let in_code = &kit.in_code;
    let t = suite.session(
        &[
            ("symbols", json!({ "attestations": attestations })),
            ("symbols", json!({ "attestations": [] })),
            ("exports", json!({ "surface": "default" })),
            ("exports", json!({ "surface": "no-such-surface" })),
            ("describeSurfaces", json!({})),
            ("inCode", json!({ "names": [in_code.absent, in_code.present], "files": in_code.files })),
            ("inCode", json!({ "names": [], "files": [] })),
        ],
        json!([]),
    )?;
    all_hold([
        t.result(3).and_then(|symbols| expect_symbols(symbols, kit)),
        t.answered(4, &json!({}), "symbols of no attestations"),
        t.result(5).and_then(|exported| expect_exports(exported, kit)),
        t.answered(6, &Value::Null, "a surface that cannot be resolved has no exports: null"),
        t.result(7).and_then(|described| expect_surfaces(described, kit)),
        t.answered(8, &json!([in_code.present]), "the names the code still holds, in the order asked"),
        t.answered(9, &json!([]), "inCode of no names"),
    ])?;
    Ok(t)
}

/// Every attestation sent has an entry, by id, and the fact names the kit's symbol.
fn expect_symbols(symbols: &Value, kit: &Kit) -> Checked {
    let symbol = &kit.facts.scalar.symbol;
    same(
        &json!(sorted_keys(symbols)),
        &json!(["e", "f", "wrong"]),
        "every attestation sent has an entry, by id",
    )?;
    let names_it = items(&symbols["f"]).iter().any(|s| s["name"] == *symbol && s["surface"] == "default");
    ensure(names_it, format!("the fact names {symbol}"))
}

fn expect_exports(exported: &Value, kit: &Kit) -> Checked {
    let exported = items(exported);
    all_hold(
        kit.exports
            .iter()
            .map(|name| ensure(exported.contains(&json!(name)), format!("default exports {name}"))),
    )
}

fn expect_surfaces(described: &Value, kit: &Kit) -> Checked {
    let mut configured: Vec<&str> = items(&kit.surfaces).iter().filter_map(|s| s["name"].as_str()).collect();
    configured.sort_unstable();
    same(&json!(sorted_keys(described)), &json!(configured), "every configured surface is described, by name")
}
