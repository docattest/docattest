//! The check of the optional capabilities: a method the greeting does not list is unknown, and
//! one it lists answers as PROTOCOL.md says.

use serde_json::{Value, json};

use super::answer::{items, texts};
use crate::conformance::expect::{Checked, ensure, same};
use crate::conformance::kit::Kit;
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::Conversation;

/// What a capability the greeting lists must answer, in the check's conversation.
type Answer = fn(&Conversation, &Kit) -> Checked;

/// The request each capability is asked by, in the conversation of the check.
const TESTS: u64 = 3;
const TESTS_OF_A_MISSING_FILE: u64 = 4;
const OBSERVABLES: u64 = 5;
const AFFECTED: u64 = 6;
const DISPLAY: u64 = 7;
/// The evaluation of the kit's expressions of the values `display` is asked to show.
const SAME_VALUES: u64 = 8;
const RUN_TESTS: u64 = 9;
const MUTANTS: u64 = 10;
/// A name no kit's project declares, which `mutants` must say it does not find.
const UNDECLARED: &str = "docattest_conformance_undeclared";

pub const CHECKS: [Check; 1] = [Check {
    name: "capabilities: a method the greeting does not list is unknown; one it lists answers",
    run: answers_what_it_lists,
}];

fn answers_what_it_lists(suite: &Suite) -> Checked<Conversation> {
    let kit = suite.kit;
    let (base, changed) = match &kit.affected {
        Some(affected) => (affected.base.clone(), json!(affected.changed)),
        None => (json!({}), json!([])),
    };
    let values: Vec<&Value> = kit.display.iter().map(|same| &same.value).collect();
    let facts: Vec<Value> = kit
        .display
        .iter()
        .enumerate()
        .map(|(k, same)| suite.requests.fact(&k.to_string(), &same.expression))
        .collect();
    let t = suite.session(
        &[
            ("tests", json!({ "file": kit.test.file })),
            ("tests", json!({ "file": "no/such/file" })),
            ("observables", json!({ "surface": "default" })),
            ("affected", json!({ "base": base, "changed": changed })),
            ("display", json!({ "values": values })),
            ("evaluate", json!({ "facts": facts })),
            ("runTests", json!({ "tests": [{ "file": kit.test.file, "describes": [], "title": kit.test.title }], "timeoutMs": 600_000 })),
            ("mutants", mutants_params(kit)),
        ],
        json!([]),
    )?;
    let listed = texts(&suite.greeting()?["capabilities"]);
    let answers: [(&str, u64, Answer); 6] = [
        ("tests", TESTS, tests_answer),
        ("runTests", RUN_TESTS, run_tests_answer),
        ("observables", OBSERVABLES, observables_answer),
        ("affected", AFFECTED, affected_answer),
        ("display", DISPLAY, display_answer),
        ("mutants", MUTANTS, mutants_answer),
    ];
    for (capability, id, answer) in answers {
        if listed.contains(&capability) { answer(&t, kit)? } else { t.failed_as(id, "internal")? }
    }
    Ok(t)
}

/// `tests`: the kit's test and suite are listed; a file that cannot be read is an answer.
fn tests_answer(t: &Conversation, kit: &Kit) -> Checked {
    let test = &kit.test;
    let listing = t.result(TESTS)?;
    let declares =
        listing["ok"] == true && items(&listing["tests"]).iter().any(|d| d["title"] == *test.title);
    ensure(declares, format!("{} declares {}", test.file, test.title))?;
    if let Some(suite) = &test.suite {
        let kind_of = |s: &Value| s["kind"].as_str().is_some_and(|kind| !kind.is_empty());
        let found = items(&listing["suites"]).iter().any(|s| s["title"] == *suite && kind_of(s));
        ensure(found, "a suite says what kind of thing it is")?;
    }
    let missing = t.result(TESTS_OF_A_MISSING_FILE)?;
    let explained = missing["ok"] == false && missing["message"].as_str().is_some_and(|m| !m.is_empty());
    ensure(explained, "a file that cannot be read is an answer, not an error")
}

/// `runTests`: the kit's project is in memory, where nothing can run, so its test is not run,
/// and the answer says why.
fn run_tests_answer(t: &Conversation, _: &Kit) -> Checked {
    let runs = items(t.result(RUN_TESTS)?);
    let explained =
        |run: &Value| run["status"] == "notRun" && run["message"].as_str().is_some_and(|m| !m.is_empty());
    ensure(
        runs.len() == 1 && explained(&runs[0]),
        "a test of a project in memory is not run, and the answer says why",
    )
}

/// `observables`: the kit's list is among what the surface offers.
fn observables_answer(t: &Conversation, kit: &Kit) -> Checked {
    let offered = items(t.result(OBSERVABLES)?)
        .iter()
        .any(|o| o["expression"] == *kit.facts.list.expression && o["value"]["items"].is_array());
    ensure(offered, "the list is among what the surface offers")
}

/// `affected`: the names the kit's change affects, when the kit has one.
fn affected_answer(t: &Conversation, kit: &Kit) -> Checked {
    let Some(affected) = &kit.affected else { return Ok(()) };
    let mut names: Vec<&str> = items(t.result(AFFECTED)?).iter().filter_map(|s| s["name"].as_str()).collect();
    names.sort_unstable();
    let mut expected = affected.names.clone();
    expected.sort();
    same(&json!(names), &json!(expected), "the names the change affects")
}

/// `display`: each of the kit's values shows as a fact of the same value does, in order.
fn display_answer(t: &Conversation, kit: &Kit) -> Checked {
    let evaluated = t.result(SAME_VALUES)?;
    let shown: Vec<&Value> =
        (0..kit.display.len()).map(|k| &evaluated[k.to_string()]["value"]["text"]).collect();
    same(t.result(DISPLAY)?, &json!(shown), "each value shows as a fact of the same value does, in order")
}

/// The kit's scalar, whose declaration has a literal in every kit, and a name nothing declares.
fn mutants_params(kit: &Kit) -> Value {
    let symbols =
        [json!({ "name": kit.facts.scalar.symbol, "surface": "default" }), json!({ "name": UNDECLARED })];
    json!({ "symbols": symbols, "limit": 8 })
}

/// `mutants`: an entry for each symbol; the scalar's first edit changes a file of the project
/// within its text, and the name nothing declares is not found.
fn mutants_answer(t: &Conversation, kit: &Kit) -> Checked {
    let answer = items(t.result(MUTANTS)?);
    ensure(answer.len() == 2, "an entry for each symbol, in order")?;
    let first = items(&answer[0]["edits"])
        .first()
        .ok_or_else(|| "the scalar's declaration has a literal to change".to_string())?;
    changes_its_file(first, kit)?;
    let undeclared = &answer[1];
    ensure(
        items(&undeclared["edits"]).is_empty() && undeclared["why"] == "notFound",
        "a name nothing declares is not found",
    )
}

/// An edit replaces bytes of a file of the kit's project, within its UTF-8 text and at
/// character boundaries, with something else.
fn changes_its_file(edit: &Value, kit: &Kit) -> Checked {
    let path = edit["path"].as_str().unwrap_or_default();
    let text = kit.files[path].as_str().ok_or_else(|| format!("{path} is a file of the project"))?;
    let (start, end) = (offset(&edit["start"]), offset(&edit["end"]));
    let inside =
        start <= end && end <= text.len() && text.is_char_boundary(start) && text.is_char_boundary(end);
    ensure(inside, format!("the edit of {path} lies within its UTF-8 text, at character boundaries"))?;
    let edited =
        format!("{}{}{}", &text[..start], edit["replacement"].as_str().unwrap_or_default(), &text[end..]);
    ensure(edited != text, "the edit changes the file")
}

/// An offset an edit gives; past any text when it is not one.
fn offset(value: &Value) -> usize {
    value.as_u64().and_then(|n| usize::try_from(n).ok()).unwrap_or(usize::MAX)
}
