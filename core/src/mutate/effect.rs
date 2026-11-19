//! What a change of code did to a proof: nothing (it still passed), made it fail (killed it),
//! or left the code unable to build or load, which tells nothing about the proof: such a
//! mutant is stillborn. The core tells the two kinds of failure apart without knowing the
//! language, from what the adapter reports: a value that differs, an assertion that fails or
//! an error about something else (a key that is gone) is how the changed code behaves; an
//! error, or an example's compile diagnostics, that points at a line of the changed file, as
//! a compiler or a loader does, is the code not building.

use crate::api::attestation::Kind;
use crate::report::{Outcome, Status};

/// What one mutant did to one proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Passed,
    Killed,
    Stillborn,
}

/// The effect of a change to `changed` (a project-relative file) on the proof whose
/// outcomes under it are `outcomes`: stillborn when every failure is the code not building.
pub fn effect_of(outcomes: &[&Outcome], changed: &str) -> Effect {
    let failures: Vec<&Outcome> = outcomes.iter().copied().filter(|o| o.is_failure()).collect();
    if failures.is_empty() {
        return Effect::Passed;
    }
    let name = changed.rsplit('/').next().unwrap_or(changed);
    if failures.iter().all(|o| does_not_build(o, name)) { Effect::Stillborn } else { Effect::Killed }
}

/// The effect of a change on a proof that passes or fails, nothing having been built.
pub fn effect_of_failing(failed: bool) -> Effect {
    if failed { Effect::Killed } else { Effect::Passed }
}

/// Whether `outcome` is the code not building: an error, or an example's compile
/// diagnostics, whose message points at a line of the file `name`.
fn does_not_build(outcome: &Outcome, name: &str) -> bool {
    let at_build =
        outcome.status == Status::Error || (outcome.kind == Kind::Example && outcome.compile_diagnostics);
    at_build && points_at(&outcome.message, name)
}

/// Whether `message` names the file `name` at a line, as compilers write a place:
/// `src/lib.rs:14:5`, `./shop.go:13:9`, `src/index.ts(3,5)`. A traceback's `"a.py", line 3`
/// is a run's, not a build's, and is not one.
fn points_at(message: &str, name: &str) -> bool {
    message.match_indices(name).any(|(at, _)| {
        let before = message[..at].chars().next_back();
        let starts = before.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.'));
        let mut after = message[at + name.len()..].chars();
        let opens = matches!(after.next(), Some(':' | '('));
        starts && opens && after.next().is_some_and(|c| c.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(kind: Kind, status: Status, message: &str) -> Outcome {
        Outcome::new("docs/a.md", 1, kind, status, message)
    }

    #[test]
    fn an_error_that_points_at_the_changed_file_is_the_code_not_building() {
        let errors = [
            "error[E0308]: mismatched types\n --> src/lib.rs:14:22",
            "./shop.go:13:9: cannot use 256 (untyped int constant) as uint8 value",
            "src/index.ts(3,5): error TS2322: Type '4' is not assignable to type '3'.",
        ];
        for (changed, message) in ["src/lib.rs", "shop.go", "src/index.ts"].into_iter().zip(errors) {
            let failed = outcome(Kind::Fact, Status::Error, message);
            assert_eq!(effect_of(&[&failed], changed), Effect::Stillborn, "{message}");
        }
        let mut diagnosed =
            outcome(Kind::Example, Status::Fail, "src/lib.rs:14:22: literal out of range (block line 1)");
        diagnosed.compile_diagnostics = true;
        assert_eq!(effect_of(&[&diagnosed], "src/lib.rs"), Effect::Stillborn);
    }

    #[test]
    fn a_value_that_differs_and_an_error_about_something_else_are_how_the_code_behaves() {
        let differs = outcome(Kind::Fact, Status::Fail, "expected 3, got 4 from N");
        let gone = outcome(Kind::Fact, Status::Error, "KeyError: 'max_attempts'");
        let traceback = outcome(Kind::Fact, Status::Error, "File \"/p/src/shop.py\", line 3, in total");
        let panicked = outcome(Kind::Example, Status::Fail, "thread 'main' panicked at src/lib.rs:30:12");
        let elsewhere = outcome(Kind::Fact, Status::Error, "src/other_lib.rs:2:1: unresolved import");
        for failed in [&differs, &gone, &traceback, &panicked, &elsewhere] {
            assert_eq!(effect_of(&[failed], "src/lib.rs"), Effect::Killed, "{}", failed.message);
        }
        let passed = outcome(Kind::Fact, Status::Pass, "N is 3");
        assert_eq!(effect_of(&[&passed], "src/lib.rs"), Effect::Passed);
        let stillborn = outcome(Kind::Fact, Status::Error, "src/lib.rs:1:1: expected item");
        assert_eq!(effect_of(&[&stillborn, &differs], "src/lib.rs"), Effect::Killed);
    }
}
