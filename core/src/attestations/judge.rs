//! The verdict on a test citation, given what its file declares.

use serde_json::Value;

use crate::adapter::Vocabulary;
use crate::api::attestation::Test;
use crate::api::errors::{Error, Result};
use crate::report::Status;

const LISTED: usize = 8;

/// A declared test, or a suite a test file declares: its title, the titles that enclose it
/// (None where one cannot be cited) and its line, as the text of the number the adapter sent.
struct Declared {
    title: Option<String>,
    describes: Vec<Option<String>>,
    line: Option<String>,
}

/// Something a test file declares that is not a test, and what the language calls it, when
/// it says.
struct Suite {
    declared: Declared,
    kind: Option<String>,
}

fn suite(value: &Value) -> Suite {
    Suite { declared: declared(value), kind: value.get("kind").and_then(Value::as_str).map(str::to_string) }
}

fn declared(value: &Value) -> Declared {
    let text = |v: &Value| v.as_str().map(str::to_string);
    Declared {
        title: value.get("title").and_then(text),
        describes: value
            .get("describes")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(text).collect())
            .unwrap_or_default(),
        line: match value.get("line") {
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        },
    }
}

/// Whether a declared test or describe block is the one cited.
fn matches(d: &Declared, cited: &Test) -> bool {
    if d.title.as_deref() != Some(cited.title.as_str()) {
        return false;
    }
    if cited.describes.is_empty() {
        return true;
    }
    d.describes.len() == cited.describes.len()
        && d.describes.iter().zip(&cited.describes).all(|(a, b)| a.as_deref() == Some(b.as_str()))
}

fn path_of(describes: &[Option<String>], title: Option<&str>, untitled: &str) -> String {
    let quote = |t: Option<&str>| t.map_or_else(|| untitled.to_string(), |t| format!("\"{t}\""));
    describes
        .iter()
        .map(|d| quote(d.as_deref()))
        .chain(std::iter::once(quote(title)))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// The verdict on a citation, given its file's `TestListing`. Every message names both.
/// What a suite is called, and what stands for a title that cannot be cited, are the
/// language's words: the listing and the adapter's vocabulary say them.
pub fn judge_citation(cited: &Test, listing: &Value, words: &Vocabulary) -> Result<(Status, String)> {
    let describes: Vec<Option<String>> = cited.describes.iter().cloned().map(Some).collect();
    let name = path_of(&describes, Some(&cited.title), &words.untitled);
    if listing.get("ok") != Some(&Value::Bool(true)) {
        let why = listing
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::internal("tests answered a failure without a message"))?;
        return Ok((Status::Error, format!("cannot cite {name}: {why}")));
    }
    let entries = |key: &str| listing.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    let tests: Vec<Declared> = entries("tests").iter().map(declared).collect();
    let shown = |d: &Declared| path_of(&d.describes, d.title.as_deref(), &words.untitled);
    if let Some(found) = tests.iter().find(|t| matches(t, cited)) {
        let Some(line) = &found.line else {
            return Err(Error::internal("tests answered a test without its line"));
        };
        let path = shown(found);
        return Ok((Status::Pass, format!("{}:{line} declares {path} (not run by docattest)", cited.file)));
    }
    let suites: Vec<Suite> = entries("suites").iter().map(suite).collect();
    let why = match suites.iter().find(|s| matches(&s.declared, cited)) {
        Some(suite) => {
            let kind = suite.kind.as_deref().map(|kind| format!("{kind}, ")).unwrap_or_default();
            format!("{name} in {} is {kind}not a test", cited.file)
        }
        None => format!("{} declares no test {name}", cited.file),
    };
    let listed = if tests.is_empty() {
        words.no_tests.clone()
    } else {
        let some: Vec<String> = tests.iter().take(LISTED).map(shown).collect();
        let more =
            if tests.len() > LISTED { format!(", and {} more", tests.len() - LISTED) } else { String::new() };
        format!("tests there: {}{more}", some.join(", "))
    };
    Ok((Status::Error, format!("{why}; {listed}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cite(title: &str, describes: &[&str]) -> (Status, String) {
        let listing = json!({ "ok": true,
            "tests": [{ "title": "throws", "describes": ["retry"], "line": 4 }, { "title": "waits", "describes": [null], "line": 9 }],
            "suites": [{ "title": "retry", "describes": [], "line": 3, "kind": "a describe block" },
                       { "title": "bare", "describes": [], "line": 7 }] });
        let t = Test {
            file: "t.test.ts".into(),
            title: title.into(),
            describes: describes.iter().map(|s| s.to_string()).collect(),
        };
        let words = Vocabulary { untitled: "(computed title)".into(), ..Vocabulary::default() };
        judge_citation(&t, &listing, &words).unwrap()
    }

    #[test]
    fn verdicts() {
        assert_eq!(
            cite("throws", &[]),
            (Status::Pass, "t.test.ts:4 declares \"retry\" > \"throws\" (not run by docattest)".into())
        );
        assert_eq!(cite("throws", &["retry"]).0, Status::Pass);
        assert_eq!(cite("throws", &["other"]).0, Status::Error);
        assert_eq!(
            cite("retry", &[]).1,
            "\"retry\" in t.test.ts is a describe block, not a test; tests there: \"retry\" > \"throws\", (computed title) > \"waits\""
        );
        assert!(cite("bare", &[]).1.starts_with("\"bare\" in t.test.ts is not a test; "));
    }

    #[test]
    fn the_words_are_the_adapters_and_a_broken_listing_is_its_bug() {
        let t = Test { file: "t.py".into(), title: "T".into(), describes: Vec::new() };
        let listing = json!({ "ok": true, "tests": [], "suites": [{ "title": "T", "describes": [], "line": 1, "kind": "a class" }] });
        let (status, message) = judge_citation(&t, &listing, &Vocabulary::default()).unwrap();
        assert_eq!(
            (status, message.as_str()),
            (Status::Error, "\"T\" in t.py is a class, not a test; it declares no test")
        );
        assert!(judge_citation(&t, &json!({ "ok": false }), &Vocabulary::default()).is_err());
        let unplaced = json!({ "ok": true, "tests": [{ "title": "T", "describes": [] }], "suites": [] });
        assert!(judge_citation(&t, &unplaced, &Vocabulary::default()).is_err());
    }
}
