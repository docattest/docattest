//! Checks of the greeting, and of the errors a confused core gets back. The first check keeps
//! the greeting it hears for the checks after it.

use serde_json::{Value, json};

use super::answer::texts;
use crate::adapter::{Greeting, PROTOCOL};
use crate::conformance::expect::{Checked, all_hold, ensure, same};
use crate::conformance::requests::{hello, request};
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::{Conversation, Line};

pub const CHECKS: [Check; 4] = [
    Check { name: "hello: answers at once, with a greeting of the protocol's shape", run: greets },
    Check { name: "hello: the answer is the same warm, and the same in every process", run: greets_alike },
    Check {
        name: "hello: another protocol number is a user error, and hello may be sent again",
        run: refuses,
    },
    Check { name: "mistakes by the core are internal errors that leave the adapter serving", run: serves_on },
];

fn greets(suite: &Suite) -> Checked<Conversation> {
    let t = suite.talk([hello().into()])?;
    let greeted = t.result(1)?;
    suite.hear(greeted);
    all_hold([
        suite.fits(greeted, "hello.result"),
        same(&greeted["protocol"], &json!(PROTOCOL), "the protocol number"),
        same(
            &greeted["adapter"]["name"],
            &json!(suite.kit.adapter),
            "the adapter names itself as its kit does",
        ),
        lower_case_languages(greeted),
        accepted_by_the_core(greeted),
        positive_globs(greeted),
        t.exited_cleanly(),
    ])?;
    Ok(t)
}

fn lower_case_languages(greeted: &Value) -> Checked {
    let languages = texts(&greeted["blockLanguages"]);
    let lower = languages.iter().all(|language| *language == language.to_lowercase());
    ensure(!languages.is_empty() && lower, "blockLanguages are lower case")
}

/// The core reads the greeting as it reads every adapter's, and refuses nothing in it.
fn accepted_by_the_core(greeted: &Value) -> Checked {
    Greeting::read(greeted).map(drop).map_err(|why| format!("the core refuses the greeting: {why}"))
}

/// The suite asks more than the core: no glob of `sourceFiles` is a negation.
fn positive_globs(greeted: &Value) -> Checked {
    let sources = &greeted["sourceFiles"];
    let globs = [texts(&sources["include"]), texts(&sources["exclude"])].concat();
    all_hold(globs.into_iter().map(|glob| ensure(!glob.starts_with('!'), "a glob does not begin with !")))
}

fn greets_alike(suite: &Suite) -> Checked<Conversation> {
    let warm = suite.talk([request(1, "hello", json!({ "protocol": PROTOCOL, "warm": true })).into()])?;
    let again = suite.talk([hello().into()])?;
    let heard = suite.greeting()?;
    all_hold([
        warm.answered(1, heard, "the greeting, warm"),
        again.answered(1, heard, "the greeting, in another process"),
    ])?;
    Ok(warm)
}

fn refuses(suite: &Suite) -> Checked<Conversation> {
    let t = suite.talk([
        request(1, "hello", json!({ "protocol": 999 })).into(),
        request(2, "hello", json!({ "protocol": PROTOCOL })).into(),
        request(3, "assertions", json!({ "examples": [] })).into(),
    ])?;
    all_hold([
        t.failed_as(1, "user"),
        t.answered(2, suite.greeting()?, "the greeting, sent again"),
        t.answered(3, &json!([]), "assertions after the greeting"),
        t.exited_cleanly(),
    ])?;
    Ok(t)
}

fn serves_on(suite: &Suite) -> Checked<Conversation> {
    let open = |id| Line::from(suite.requests.open_as(id, json!([])));
    let t = suite.talk([
        request(1, "assertions", json!({ "examples": [] })).into(), // before hello
        request(2, "hello", json!({ "protocol": PROTOCOL })).into(),
        request(3, "no-such-method", json!({})).into(),
        request(4, "exports", json!({ "surface": "default" })).into(), // before open
        request(5, "assertions", json!({ "examples": 3 })).into(),     // params of the wrong shape
        Line::Text("{ this is not JSON"),
        Line::Text(r#"{"method":"assertions","params":{"examples":[]}}"#), // no id
        Line::Text(""),
        open(6),
        open(7), // a second open
        request(8, "assertions", json!({ "examples": [] })).into(),
    ])?;
    all_hold([
        all_hold([1, 3, 4, 5, 7].map(|id| t.failed_as(id, "internal"))),
        unread_lines_answered(&t),
        t.answered(6, &json!({}), "the open after the mistakes"),
        t.answered(8, &json!([]), "assertions after the mistakes"),
        same(&json!(t.out.len()), &json!(10), "one answer for each request, none for the blank line"),
        t.exited_cleanly(),
    ])?;
    Ok(t)
}

/// A line that cannot be read, and one without an id, are each answered with `"id": null`.
fn unread_lines_answered(t: &Conversation) -> Checked {
    let unread = t.responses.iter().filter(|r| r["id"].is_null() && r["error"]["kind"] == "internal").count();
    ensure(unread == 2, "a line that cannot be read, and one without an id, are answered with \"id\": null")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_greeting_the_core_refuses_does_not_conform() {
        let greeting = |ignored: &str| {
            json!({
                "blockLanguages": ["x"], "capabilities": [], "ignoredNames": [ignored],
                "sourceFiles": { "include": ["**/*.x"], "exclude": [] },
            })
        };
        assert_eq!(accepted_by_the_core(&greeting("build")), Ok(()));
        let refused = accepted_by_the_core(&greeting("a/b")).unwrap_err();
        assert!(refused.starts_with("the core refuses the greeting: \"ignoredNames\""), "{refused}");
    }
}
