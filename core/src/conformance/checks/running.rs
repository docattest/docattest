//! Checks of running: a cases table, stdout kept clean, the order answers come in, and the end
//! of input.

use serde_json::{Value, json};

use super::answer::items;
use crate::conformance::expect::{Checked, all_hold, ensure, same};
use crate::conformance::requests::{hello, request};
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::Conversation;

/// The line of the cases table on the page; its rows start two lines below it.
const TABLE_LINE: u64 = 100;
const FIRST_ROW_LINE: u64 = TABLE_LINE + 2;

/// Only when the kit has a cases table.
pub const CASES: Check =
    Check { name: "run: a cases table gives one result for each row, at the row's line", run: runs_cases };

pub const CHECKS: [Check; 3] = [
    Check {
        name: "stdout carries nothing but responses, whatever attested code prints",
        run: keeps_stdout_clean,
    },
    Check {
        name: "answers come as requests finish; close waits for what was sent before it",
        run: answers_as_done,
    },
    Check {
        name: "at end of input the answers still in flight are written, and the adapter exits 0",
        run: finishes_in_flight,
    },
];

/// The first result of the `run` that is request `id` passed.
fn passed(t: &Conversation, id: u64, what: &str) -> Checked {
    let answer = t.result(id)?;
    let first = items(answer).first().ok_or_else(|| format!("a run with no results: {answer}"))?;
    same(&first["status"], &json!("pass"), what)
}

fn runs_cases(suite: &Suite) -> Checked<Conversation> {
    let Some(cases) = &suite.kit.cases else { return Err("the kit has no cases table".to_string()) };
    let rows: Vec<Value> = cases
        .rows
        .iter()
        .zip(FIRST_ROW_LINE..)
        .map(|(values, line)| {
            let name = cases.header.first().and_then(|column| values.get(column));
            json!({ "line": line, "name": name, "values": values })
        })
        .collect();
    let table = json!({
        "id": "c", "page": suite.requests.page(), "line": TABLE_LINE, "claim": "", "kind": "cases",
        "runner": cases.runner, "header": cases.header, "rows": rows,
    });
    let t = suite.session(&[("run", json!({ "examples": [], "cases": [table] }))], json!([]))?;
    let verdicts: Vec<Value> =
        items(t.result(3)?).iter().map(|r| json!([r["line"], r["kind"], r["status"]])).collect();
    let expected: Vec<Value> = cases
        .statuses
        .iter()
        .zip(FIRST_ROW_LINE..)
        .map(|(status, line)| json!([line, "cases", status]))
        .collect();
    same(&json!(verdicts), &json!(expected), "a result for each row, at its line")?;
    Ok(t)
}

fn keeps_stdout_clean(suite: &Suite) -> Checked<Conversation> {
    let prints = suite.requests.example("prints", &suite.kit.examples.prints);
    let t = suite.session(&[("run", json!({ "examples": [prints], "cases": [] }))], json!([prints]))?;
    let said = format!("three requests, three lines on stdout:\n{}", t.out.join("\n"));
    all_hold([passed(&t, 3, "the example that prints"), ensure(t.out.len() == 3, said)])?;
    Ok(t)
}

fn answers_as_done(suite: &Suite) -> Checked<Conversation> {
    let (requests, examples) = (&suite.requests, &suite.kit.examples);
    let slow = requests.example("slow", &examples.slow);
    let quick = requests.example("quick", &examples.passes);
    let t = suite.talk([
        hello().into(),
        requests.open(json!([slow, quick])).into(),
        request(3, "run", json!({ "examples": [slow], "cases": [] })).into(),
        request(4, "assertions", json!({ "examples": [] })).into(),
        request(5, "close", json!({})).into(),
        request(6, "exports", json!({ "surface": "default" })).into(), // after close
    ])?;
    all_hold([
        passed(&t, 3, "the slow example"),
        t.answered(5, &json!({}), "close"),
        t.answered_before(4, 3, "a quick answer overtakes a slow one"),
        t.answered_before(3, 5, "close is answered after the run sent before it"),
        t.failed_as(6, "internal"),
        t.exited_cleanly(),
    ])?;
    Ok(t)
}

fn finishes_in_flight(suite: &Suite) -> Checked<Conversation> {
    let slow = suite.requests.example("slow", &suite.kit.examples.slow);
    let run = ("run", json!({ "examples": [slow], "cases": [] }));
    let t = suite.session(&[run.clone(), run], json!([slow]))?;
    let said = "both runs in flight when the input ended";
    all_hold([passed(&t, 3, said), passed(&t, 4, said), t.exited_cleanly()])?;
    Ok(t)
}
