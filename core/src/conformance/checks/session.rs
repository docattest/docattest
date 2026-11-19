//! Checks of a session's methods: compile and run, evaluate, and what the index and the
//! report need.

use serde_json::{Value, json};

use super::answer::{items, sorted_keys};
use crate::conformance::expect::{Checked, all_hold, ensure, same};
use crate::conformance::kit::{Facts, Kit};
use crate::conformance::requests::{
    FIELD, FIELD_VALUE, MANIFEST, MANIFEST_VERSION, PATTERN, PATTERN_MATCHES, TEXT, TEXT_FILE,
};
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::Conversation;
use crate::fact_files::calls::Reader;

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
        requests.fact("file", &Reader::File.call(TEXT_FILE)),
        requests.fact("unread-file", &Reader::File.call(UNREAD_TEXT)),
        requests.fact("matched", &two_literals("matched", TEXT_FILE, PATTERN)),
        requests.fact("unread-match", &two_literals("matched", TEXT_FILE, UNREAD_ARGUMENT)),
        requests.fact("field", &two_literals("manifestField", MANIFEST, FIELD)),
        requests.fact("unread-field", &two_literals("manifestField", MANIFEST, UNREAD_ARGUMENT)),
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

/// A manifest and a file the session was not opened with, and a second argument of
/// `matched` and of `manifestField` that it was not.
const UNREAD: &str = "unread.json";
const UNREAD_TEXT: &str = "unread.txt";
const UNREAD_ARGUMENT: &str = "unread.argument";

/// A call of `function` with two arguments, each in double quotes, as every language writes
/// a string that holds no quote or backslash.
fn two_literals(function: &str, path: &str, argument: &str) -> String {
    format!("{function}(\"{path}\", \"{argument}\")")
}

/// A value for every fact, each displayed as the kit says; none for the invalid one, nor for
/// a file the session does not have.
fn expect_values(values: &Value, facts: &Facts) -> Checked {
    let (scalar, text, invalid) = (&values["scalar"]["value"], &values["text"]["value"], &values["invalid"]);
    all_hold([
        same(
            &json!(sorted_keys(values)),
            &json!([
                "field",
                "file",
                "invalid",
                "list",
                "manifest",
                "matched",
                "scalar",
                "text",
                "unread",
                "unread-field",
                "unread-file",
                "unread-match"
            ]),
            "a value for every fact",
        ),
        expect_files_read(values),
        expect_values_read(values),
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

/// A manifest's field and a file's text, as the session was opened with them; a failure that
/// names the file for each it was not.
fn expect_files_read(values: &Value) -> Checked {
    let (manifest, unread) = (&values["manifest"]["value"], &values["unread"]);
    let (file, unread_file) = (&values["file"]["value"], &values["unread-file"]);
    let names = |failure: &Value, file: &str| failure["message"].as_str().is_some_and(|m| m.contains(file));
    all_hold([
        same(
            &json!([manifest["text"], manifest["kind"]]),
            &json!([MANIFEST_VERSION, "text"]),
            "a manifest's field",
        ),
        same(&unread["ok"], &json!(false), "a manifest the session was not opened with"),
        ensure(names(unread, UNREAD), "a manifest that cannot be read is named in the message"),
        same(&json!([file["text"], file["kind"]]), &json!([TEXT, "text"]), "a file's text, which is text"),
        same(&unread_file["ok"], &json!(false), "a file the session was not opened with"),
        ensure(names(unread_file, UNREAD_TEXT), "a file that cannot be read is named in the message"),
    ])
}

/// What `matched` and `manifestField` give, as the session was opened with them: a match is
/// text, a field's integer is displayed as the language displays one; a failure that names
/// both arguments for each the session was not opened with.
fn expect_values_read(values: &Value) -> Checked {
    let (matched, unread_match) = (&values["matched"]["value"], &values["unread-match"]);
    let (field, unread_field) = (&values["field"]["value"], &values["unread-field"]);
    let names = |failure: &Value, file: &str| {
        failure["message"].as_str().is_some_and(|m| m.contains(file) && m.contains(UNREAD_ARGUMENT))
    };
    all_hold([
        same(
            &json!([matched["text"], matched["kind"]]),
            &json!([PATTERN_MATCHES, "text"]),
            "what a regex matches, which is text",
        ),
        same(
            &field["text"],
            &json!(FIELD_VALUE.to_string()),
            "a manifest's field, displayed as an integer is",
        ),
        ensure(field.get("kind").is_none(), "a field that holds a number is not text"),
        same(&unread_match["ok"], &json!(false), "a match the session was not opened with"),
        ensure(names(unread_match, TEXT_FILE), "a match that was not read names its file and regex"),
        same(&unread_field["ok"], &json!(false), "a field the session was not opened with"),
        ensure(names(unread_field, MANIFEST), "a field that was not read names its file and path"),
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
