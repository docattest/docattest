//! The check of `problems` (PROTOCOL.md §3): a method the greeting does not list is unknown,
//! and when it lists it, each problem it answers is at a file of the project, by a path
//! relative to the root, as the core reports it.

use serde_json::json;

use super::answer::{items, texts};
use crate::conformance::expect::{Checked, all_hold, ensure};
use crate::conformance::suite::{Check, Suite};
use crate::conformance::talk::Conversation;

/// The capability, and the method.
const PROBLEMS: &str = "problems";
/// The request `problems` is asked by, in the conversation of the check.
const ASKED: u64 = 3;

pub const CHECKS: [Check; 1] = [Check {
    name: "problems: unknown unless the greeting lists it; each problem is at a file of the project",
    run: answers_problems,
}];

fn answers_problems(suite: &Suite) -> Checked<Conversation> {
    let t = suite.session(&[(PROBLEMS, json!({}))], json!([]))?;
    if !texts(&suite.greeting()?["capabilities"]).contains(&PROBLEMS) {
        t.failed_as(ASKED, "internal")?;
        return Ok(t);
    }
    let problems = items(&t.result(ASKED)?["problems"]);
    all_hold(problems.iter().map(|problem| {
        let path = problem["path"].as_str().unwrap_or_default();
        ensure(is_project_path(path), format!("{problem} is at a path relative to the project root"))
    }))?;
    Ok(t)
}

/// Whether `path` is relative to the project root with `/` separators and never leaves it, as
/// every path of the protocol is (PROTOCOL.md §2).
fn is_project_path(path: &str) -> bool {
    !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path.split('/').all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_is_at_a_path_inside_the_project() {
        for inside in ["pkg/helpers_docattest.go", "go.mod", ".config/lint.toml"] {
            assert!(is_project_path(inside), "{inside}");
        }
        for outside in
            ["/abs/a.go", "../a.go", "pkg/../a.go", "./a.go", "pkg\\a.go", "C:/a.go", "", "pkg//a.go"]
        {
            assert!(!is_project_path(outside), "{outside}");
        }
    }
}
