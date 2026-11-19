//! What the adapter finds wrong in the project outside every attestation (PROTOCOL.md §3,
//! `problems`): failures at a file and line, such as what a linter finds in a file only
//! docattest builds, and warnings about the whole project. The core asks once per check, of an
//! adapter that lists the capability; an adapter written before it existed is asked nothing.

use serde_json::{Value, json};

use crate::adapter::{Adapter, Pending};
use crate::api::attestation::Kind;
use crate::api::errors::{Error, Result};
use crate::report::{Outcome, Status};

/// The capability, and the method, by which an adapter reports them.
const PROBLEMS: &str = "problems";

/// Asks for the project's problems, of an adapter that lists the capability.
pub(super) fn send_problems(adapter: &Adapter) -> Option<Pending> {
    adapter.can(PROBLEMS).then(|| adapter.send(PROBLEMS, json!({})))
}

/// What the adapter found: each problem as a failing result at its file and line, and the
/// project's warnings. Nothing when it was not asked.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Problems {
    pub results: Vec<Outcome>,
    pub warnings: Vec<String>,
}

impl Problems {
    /// The answer to [`send_problems`]. One that does not fit the schema is the adapter's bug.
    pub(super) fn wait(pending: Option<Pending>) -> Result<Problems> {
        let Some(pending) = pending else { return Ok(Problems::default()) };
        Problems::from_answer(&pending.wait()?)
    }

    fn from_answer(answer: &Value) -> Result<Problems> {
        let malformed =
            || Error::internal("problems did not answer a list of problems and a list of warnings");
        let problems = answer.get("problems").and_then(Value::as_array).ok_or_else(malformed)?;
        let warnings = answer.get("warnings").and_then(Value::as_array).ok_or_else(malformed)?;
        Ok(Problems {
            results: problems.iter().map(failure).collect::<Result<_>>()?,
            warnings: warnings
                .iter()
                .map(|w| w.as_str().map(str::to_string).ok_or_else(malformed))
                .collect::<Result<_>>()?,
        })
    }
}

/// A problem as the result that reports it: a failure at its file and line.
fn failure(problem: &Value) -> Result<Outcome> {
    let text = |key: &str| problem.get(key).and_then(Value::as_str);
    let path = text("path").filter(|p| !p.is_empty());
    let line = problem.get("line").and_then(Value::as_u64).filter(|&l| l > 0);
    let (Some(path), Some(line), Some(message)) = (path, line, text("message")) else {
        return Err(Error::internal("problems answered a problem without a path, a line or a message"));
    };
    Ok(Outcome::new(path, line as usize, Kind::Problem, Status::Fail, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_problem_is_a_failure_at_its_file_and_line() {
        let answer = json!({
            "problems": [{ "path": "pkg/helpers_docattest.go", "line": 7, "message": "go vet: wrong type" }],
            "warnings": ["go.mod requires another release"],
        });
        assert_eq!(
            Problems::from_answer(&answer).unwrap(),
            Problems {
                results: vec![Outcome::new(
                    "pkg/helpers_docattest.go",
                    7,
                    Kind::Problem,
                    Status::Fail,
                    "go vet: wrong type"
                )],
                warnings: vec!["go.mod requires another release".into()],
            }
        );
    }

    #[test]
    fn an_answer_that_does_not_fit_the_schema_is_the_adapters_bug() {
        let problem = json!({ "path": "a.go", "line": 1, "message": "m" });
        for wrong in [
            json!({ "problems": [] }),
            json!({ "warnings": [] }),
            json!({ "problems": [], "warnings": [3] }),
            json!({ "problems": [{ "path": "", "line": 1, "message": "m" }], "warnings": [] }),
            json!({ "problems": [{ "path": "a.go", "line": 0, "message": "m" }], "warnings": [] }),
            json!({ "problems": [{ "path": "a.go", "line": 1 }], "warnings": [] }),
        ] {
            assert!(matches!(Problems::from_answer(&wrong), Err(Error::Internal { .. })), "{wrong}");
        }
        assert!(Problems::from_answer(&json!({ "problems": [problem], "warnings": [] })).is_ok());
    }
}
