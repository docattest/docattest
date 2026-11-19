//! `check --run-cited`: the tests that the citations which hold name, run by the adapter
//! (`runTests`, PROTOCOL.md §3), and what each run makes of its citation.

use serde_json::{Value, json};

use crate::adapter::{Adapter, Pending};
use crate::api::attestation::{Attestation, Kind};
use crate::api::errors::{Error, Result};
use crate::attestations::judge::CitedTest;
use crate::report::{Outcome, Status};

/// Whether a check runs the tests that citations name, or only reads that they are declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitedTests {
    Declared,
    Run,
}

/// How long one run of a test runner may take. A test suite is not a block, so this is not
/// `runtime.timeoutMs`; it only stops a run that hangs.
const RUN_TIMEOUT_MS: u64 = 600_000;
/// The most lines of what a runner said that a message shows.
const EXCERPT_LINES: usize = 10;

/// `--run-cited` needs an adapter that runs tests; one that does not is a user error before
/// anything runs, whether or not the project has citations, so the flag never passes for
/// nothing.
pub(super) fn refuse_without_runner(adapter: &Adapter, cited: CitedTests) -> Result<()> {
    if cited == CitedTests::Declared || adapter.can("runTests") {
        return Ok(());
    }
    Err(Error::user(format!(
        "--run-cited runs the tests that citations name, and the {} adapter does not run tests",
        adapter.name()
    )))
}

/// A citation that holds, waiting for its test's run: `at` is the test's place in the request.
struct Waiting {
    citation: Attestation,
    file: String,
    test: CitedTest,
    at: usize,
}

/// The tests the citations that hold name, each once, and the citations waiting for them.
#[derive(Default)]
pub(super) struct CitedRuns {
    tests: Vec<Value>,
    waiting: Vec<Waiting>,
}

impl CitedRuns {
    /// Wait for the run of `test`, which `citation` names in `file`. A test with a title around
    /// it that the file does not write cannot be selected in any language: its outcome is
    /// returned at once instead.
    pub(super) fn add(&mut self, citation: &Attestation, file: &str, test: CitedTest) -> Option<Outcome> {
        let Some(describes) = test.describes.iter().cloned().collect::<Option<Vec<String>>>() else {
            let why = "cannot be run: a title around it is not written in the file";
            return Some(outcome(citation, Status::Error, &said(file, &test, why)));
        };
        let request = json!({ "file": file, "describes": describes, "title": test.title });
        let at = self.tests.iter().position(|t| *t == request).unwrap_or_else(|| {
            self.tests.push(request);
            self.tests.len() - 1
        });
        self.waiting.push(Waiting { citation: citation.clone(), file: file.to_string(), test, at });
        None
    }

    /// Ask the adapter to run the tests; nothing is asked when no citation waits.
    pub(super) fn send(self, adapter: &Adapter) -> Option<SentRuns> {
        if self.tests.is_empty() {
            return None;
        }
        let pending = adapter.send("runTests", json!({ "tests": self.tests, "timeoutMs": RUN_TIMEOUT_MS }));
        Some(SentRuns { waiting: self.waiting, asked: self.tests.len(), pending })
    }
}

/// The run the adapter was asked for, and the citations waiting for it.
pub(super) struct SentRuns {
    waiting: Vec<Waiting>,
    asked: usize,
    pending: Pending,
}

/// What one run made of its citation, and whether its test ran at all.
pub(super) struct RunOutcome {
    pub(super) citation: Attestation,
    pub(super) outcome: Outcome,
    pub(super) ran: bool,
}

impl SentRuns {
    /// The outcome of each waiting citation, in the order they were added.
    pub(super) fn outcomes(self) -> Result<Vec<RunOutcome>> {
        let answer = self.pending.wait()?;
        let runs = match answer {
            Value::Array(runs) if runs.len() == self.asked => runs,
            _ => {
                let asked = self.asked;
                return Err(Error::internal(format!(
                    "runTests did not answer one run for each of {asked} tests"
                )));
            }
        };
        self.waiting
            .into_iter()
            .map(|w| {
                let (status, why, ran) = verdict(&runs[w.at])?;
                let outcome = outcome(&w.citation, status, &said(&w.file, &w.test, &why));
                Ok(RunOutcome { citation: w.citation, outcome, ran })
            })
            .collect()
    }
}

/// A run's status for its citation, the end of its message, and whether its test ran.
fn verdict(run: &Value) -> Result<(Status, String, bool)> {
    let status = run.get("status").and_then(Value::as_str);
    let message = || {
        run.get("message")
            .and_then(Value::as_str)
            .map(excerpt)
            .ok_or_else(|| Error::internal("runTests answered a run that did not pass without a message"))
    };
    Ok(match status {
        Some("passed") => (Status::Pass, "ran and passed".to_string(), true),
        Some("failed") => (Status::Fail, format!("failed: {}", message()?), true),
        Some("skipped") => (Status::Error, format!("was skipped: {}", message()?), false),
        Some("notRun") => (Status::Error, format!("did not run: {}", message()?), false),
        _ => return Err(Error::internal("runTests answered a run without a status it knows")),
    })
}

/// What a runner said, trimmed, in at most [`EXCERPT_LINES`] lines.
fn excerpt(message: &str) -> String {
    let lines: Vec<&str> = message.trim().lines().collect();
    if lines.len() <= EXCERPT_LINES {
        return lines.join("\n");
    }
    format!("{}\n…", lines[..EXCERPT_LINES].join("\n"))
}

/// `<file>:<line> declares <path>, which <why>`: the declaration a citation holds by, and what
/// running it gave.
fn said(file: &str, test: &CitedTest, why: &str) -> String {
    format!("{file}:{} declares {}, which {why}", test.line, test.path)
}

fn outcome(citation: &Attestation, status: Status, message: &str) -> Outcome {
    Outcome::new(&citation.page, citation.line, Kind::Test, status, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_status_of_a_run_is_a_verdict_on_its_citation() {
        let run = |v: Value| verdict(&v).unwrap();
        assert_eq!(run(json!({ "status": "passed" })), (Status::Pass, "ran and passed".into(), true));
        assert_eq!(
            run(json!({ "status": "failed", "message": "  expected 3\n  got 4\n" })),
            (Status::Fail, "failed: expected 3\n  got 4".into(), true)
        );
        assert_eq!(
            run(json!({ "status": "skipped", "message": "not on Windows" })),
            (Status::Error, "was skipped: not on Windows".into(), false)
        );
        assert_eq!(
            run(json!({ "status": "notRun", "message": "no test matched" })),
            (Status::Error, "did not run: no test matched".into(), false)
        );
        assert!(verdict(&json!({ "status": "failed" })).is_err());
        assert!(verdict(&json!({ "status": "lost" })).is_err());
    }

    fn citation(line: usize) -> Attestation {
        let body = crate::api::attestation::Body::Test(crate::api::attestation::Test::default());
        Attestation {
            id: format!("c{line}"),
            page: "docs/a.md".into(),
            line,
            claim: String::new(),
            error: None,
            body,
        }
    }

    fn declared(describes: Vec<Option<&str>>) -> CitedTest {
        let describes = describes.into_iter().map(|d| d.map(str::to_string)).collect();
        CitedTest { describes, title: "waits".into(), line: "9".into(), path: "\"waits\"".into() }
    }

    #[test]
    fn a_test_two_citations_name_is_asked_for_once() {
        let mut runs = CitedRuns::default();
        assert!(runs.add(&citation(1), "t.test.ts", declared(vec![Some("retry")])).is_none());
        assert!(runs.add(&citation(3), "t.test.ts", declared(vec![Some("retry")])).is_none());
        assert!(runs.add(&citation(5), "u.test.ts", declared(vec![Some("retry")])).is_none());
        assert_eq!(
            runs.tests,
            [
                json!({ "file": "t.test.ts", "describes": ["retry"], "title": "waits" }),
                json!({ "file": "u.test.ts", "describes": ["retry"], "title": "waits" })
            ]
        );
        assert_eq!(runs.waiting.iter().map(|w| w.at).collect::<Vec<_>>(), [0, 0, 1]);
    }

    #[test]
    fn a_test_inside_a_title_the_file_does_not_write_is_not_asked_for() {
        let mut runs = CitedRuns::default();
        let refused = runs.add(&citation(1), "t.test.ts", declared(vec![Some("retry"), None])).unwrap();
        assert_eq!(refused.status, Status::Error);
        assert_eq!(
            refused.message,
            "t.test.ts:9 declares \"waits\", which cannot be run: a title around it is not written in the file"
        );
        assert!(runs.tests.is_empty() && runs.waiting.is_empty());
    }

    #[test]
    fn a_long_message_is_cut_to_its_first_lines() {
        let long: Vec<String> = (1..=12).map(|n| format!("line {n}")).collect();
        let cut = excerpt(&long.join("\n"));
        assert_eq!(cut.lines().count(), EXCERPT_LINES + 1);
        assert!(cut.starts_with("line 1\n") && cut.ends_with("line 10\n…"));
        assert_eq!(excerpt("one\ntwo"), "one\ntwo");
    }
}
