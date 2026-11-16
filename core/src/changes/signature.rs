//! What an attestation is, independent of where it sits. Built generically from its JSON form, so a
//! kind added later is compared by whatever it carries.

use serde_json::{Map, Value};

use super::text::squash;
use crate::api::attestation::{Attestation, Body, FactTarget, target_json};
use crate::text::compare_utf16;

// Where it is, what it says, how it ran, and the page side of a fact: none of these is the
// proof itself. `mode` and `weak` are left out so a downgraded block still matches.
const NOT_PROOF: [&str; 8] = ["id", "page", "line", "claim", "error", "mode", "weak", "target"];

/// The proof an attestation carries: code, expression, runner and rows, citation, …
///
/// Proofs are only compared for equality and split into words, so the key order here
/// (sorted) never shows.
pub fn proof_of(a: &Attestation) -> String {
    canonical(&a.to_json(), &NOT_PROOF).to_string()
}

/// The page side of a fact (its literal, list, column or block); '' for other kinds.
pub fn target_of(a: &Attestation) -> String {
    let Body::Fact(fact) = &a.body else { return String::new() };
    let Ok(target) = &fact.target else { return String::new() };
    let mut value = target_json(target);
    if let FactTarget::Set { items, .. } = target
        && !fact.ordered
    {
        // Sorted as written, before squashing: an unordered set compares in the order of its
        // items' spelling on the page, spacing included.
        let mut sorted = items.clone();
        sorted.sort_by(|x, y| compare_utf16(x, y));
        if let Some(object) = value.as_object_mut() {
            object.insert("items".into(), Value::from(sorted));
        }
    }
    canonical(&value, &[]).to_string()
}

fn canonical(value: &Value, skip: &[&str]) -> Value {
    match value {
        Value::String(s) => Value::String(squash(s)),
        Value::Array(items) => Value::Array(items.iter().map(|v| canonical(v, skip)).collect()),
        Value::Object(object) => {
            // Nested lines (a cases row's) are positions too.
            let mut entries: Vec<(&String, &Value)> =
                object.iter().filter(|(key, _)| !skip.contains(&key.as_str()) && *key != "line").collect();
            entries.sort_by(|x, y| compare_utf16(x.0, y.0));
            let mut out = Map::new();
            for (key, v) in entries {
                out.insert(key.clone(), canonical(v, skip));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::attestation::{Cases, CasesRow, Example, Fact, Mode, SetFrom};

    fn at(line: usize, body: Body) -> Attestation {
        Attestation {
            id: format!("p:{line}"),
            page: "p.md".into(),
            line,
            claim: "c".into(),
            error: None,
            body,
        }
    }

    fn fact(expression: &str, ordered: bool, target: Result<FactTarget, String>) -> Body {
        Body::Fact(Fact {
            expression: expression.into(),
            surface: "default".into(),
            ordered,
            column: None,
            target,
        })
    }

    #[test]
    fn a_proof_ignores_position_mode_and_whitespace() {
        let example = |mode, weak, code: &str| {
            Body::Example(Example {
                lang: "ts".into(),
                mode,
                network: false,
                code: code.into(),
                weak,
                owner: None,
                tags: Vec::new(),
            })
        };
        let a = at(3, example(Mode::Check, false, "assert.ok(1);\n  x();"));
        let b = at(9, example(Mode::Compile, true, "assert.ok(1); x();"));
        assert_eq!(proof_of(&a), proof_of(&b));
        assert_eq!(
            proof_of(&a),
            r#"{"code":"assert.ok(1); x();","kind":"example","lang":"ts","network":false}"#
        );
        let c = at(3, example(Mode::Check, false, "assert.ok(2);"));
        assert_ne!(proof_of(&a), proof_of(&c));
    }

    #[test]
    fn a_fact_proof_is_its_expression_not_its_target() {
        let a = at(1, fact("LIMIT", false, Ok(FactTarget::Scalar("3".into()))));
        let b = at(1, fact("LIMIT", false, Ok(FactTarget::Scalar("5".into()))));
        assert_eq!(proof_of(&a), proof_of(&b));
        assert_eq!(
            proof_of(&a),
            r#"{"expression":"LIMIT","kind":"fact","ordered":false,"surface":"default"}"#
        );
        assert_ne!(target_of(&a), target_of(&b));
        assert_eq!(target_of(&a), r#"{"kind":"scalar","value":"3"}"#);
        assert_eq!(target_of(&at(1, fact("", false, Err("no target".into())))), "");
    }

    #[test]
    fn an_unordered_set_target_is_sorted_and_an_ordered_one_is_not() {
        let set = |items: &[&str]| FactTarget::Set {
            items: items.iter().map(|s| s.to_string()).collect(),
            from: SetFrom::List,
        };
        let unordered = |items: &[&str]| target_of(&at(1, fact("K", false, Ok(set(items)))));
        let ordered = |items: &[&str]| target_of(&at(1, fact("K", true, Ok(set(items)))));
        assert_eq!(unordered(&["b", "a"]), unordered(&["a", "b"]));
        assert_ne!(ordered(&["b", "a"]), ordered(&["a", "b"]));
        assert_eq!(ordered(&["a", "b"]), unordered(&["b", "a"]));
    }

    #[test]
    fn rows_are_compared_without_their_lines_and_skipped_names_at_any_depth() {
        let cases = |line: usize, id: &str| {
            Body::Cases(Cases {
                runner: "echo".into(),
                header: vec!["case".into(), "id".into()],
                rows: vec![CasesRow {
                    line,
                    name: "one".into(),
                    values: vec![("case".into(), "one".into()), ("id".into(), id.into())],
                }],
            })
        };
        // A column named like a skipped field is skipped inside the row too: the skip
        // applies at every depth.
        assert_eq!(proof_of(&at(3, cases(6, "1"))), proof_of(&at(8, cases(11, "2"))));
        assert_eq!(
            proof_of(&at(3, cases(6, "1"))),
            r#"{"header":["case","id"],"kind":"cases","rows":[{"name":"one","values":{"case":"one"}}],"runner":"echo"}"#
        );
    }
}
