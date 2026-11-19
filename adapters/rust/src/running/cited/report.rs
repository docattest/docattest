//! What `cargo test` prints, read: the tests libtest's `--list` lists, how each test of a
//! run ended, and what a build that failed said first. Stable libtest has no
//! machine-readable report, so its text is read, line by line as it prints them.

use crate::protocol::TestRun;
use crate::toolchain::cargo::last_error;
use std::collections::HashMap;

/// How libtest reported a test it ran.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Reported {
    Passed,
    /// With the failure as libtest printed it.
    Failed(String),
    /// With the reason `#[ignore = "…"]` gives, when it gives one.
    Ignored(Option<String>),
}

/// What libtest printed of one run.
#[derive(Debug, Default)]
pub(super) struct Report {
    pub(super) tests: HashMap<String, Reported>,
    /// Whether it printed its summary, `test result: …`: a run whose test program ended
    /// early (a test that aborts the process) does not.
    pub(super) finished: bool,
}

/// What a test that libtest ignored says, when `#[ignore]` gives no reason.
const IGNORED: &str = "cargo test ignored it: it is marked #[ignore]";

/// The names of the tests `--list` lists.
pub(super) fn listed(stdout: &str) -> Vec<String> {
    stdout.lines().filter_map(|line| line.strip_suffix(": test")).map(String::from).collect()
}

/// Reads what libtest printed of a run: `test <name> ... ok`, `FAILED` or
/// `ignored[, <reason>]` for each test, and each failure's own output below.
pub(super) fn report(stdout: &str) -> Report {
    let failures = failures(stdout);
    let mut tests = HashMap::new();
    for line in stdout.lines() {
        let Some((name, result)) = line.strip_prefix("test ").and_then(|rest| rest.rsplit_once(" ... "))
        else {
            continue;
        };
        let name = name.strip_suffix(" - should panic").unwrap_or(name);
        let reported = match result {
            "ok" => Reported::Passed,
            "FAILED" => {
                Reported::Failed(failures.get(name).cloned().unwrap_or_else(|| "it failed".to_string()))
            }
            "ignored" => Reported::Ignored(None),
            _ => match result.strip_prefix("ignored, ") {
                Some(reason) => Reported::Ignored(Some(reason.to_string())),
                None => continue,
            },
        };
        tests.insert(name.to_string(), reported);
    }
    Report { tests, finished: stdout.lines().any(|line| line.starts_with("test result: ")) }
}

/// The output of each failed test, by name: the block under `---- <name> stdout ----`.
fn failures(stdout: &str) -> HashMap<String, String> {
    let mut blocks: HashMap<String, Vec<&str>> = HashMap::new();
    let mut current: Option<&str> = None;
    for line in stdout.lines() {
        if let Some(name) = line.strip_prefix("---- ").and_then(|rest| rest.strip_suffix(" stdout ----")) {
            current = Some(name);
            blocks.entry(name.to_string()).or_default();
        } else if line == "failures:" {
            current = None;
        } else if let Some(name) = current {
            blocks.entry(name.to_string()).or_default().push(line);
        }
    }
    blocks.into_iter().map(|(name, block)| (name, failure_message(&block))).collect()
}

/// A failure's own lines: from its panic on, when it panicked, since what the test printed
/// before is not the failure; without libtest's hint about backtraces.
fn failure_message(block: &[&str]) -> String {
    let from = block.iter().position(|line| line.starts_with("thread '") && line.contains("panicked at"));
    let kept: Vec<&str> = block[from.unwrap_or(0)..]
        .iter()
        .copied()
        .filter(|line| !line.starts_with("note: run with `RUST_BACKTRACE=1`"))
        .collect();
    let message = kept.join("\n").trim().to_string();
    if message.is_empty() { "it failed, and printed nothing".to_string() } else { message }
}

/// What a build that failed said first: its first error, to the blank line that ends it.
pub(super) fn first_error(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().collect();
    let Some(start) = lines.iter().position(|line| line.starts_with("error")) else {
        return last_error(stderr);
    };
    let length = lines[start..].iter().position(|line| line.trim().is_empty()).unwrap_or(lines.len() - start);
    lines[start..start + length].join("\n")
}

/// How many of the last lines cargo printed say why a test program ended early: cargo's
/// error, and the status the program ended with.
const ENDED_LINES: usize = 3;

/// Why a run did not finish: the last lines cargo printed, which say how its test program
/// ended (`process didn't exit successfully: … (signal: 6, SIGABRT: process abort signal)`).
pub(super) fn ended_early(stderr: &str) -> String {
    let said: Vec<&str> = stderr.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let last = &said[said.len().saturating_sub(ENDED_LINES)..];
    format!("the test program ended before it reported every test:\n{}", last.join("\n"))
}

/// How a cited test ran, from the report of the run of its tests, by their names: it
/// passed when every one passed. `cut_short` says why a run that did not finish ended.
pub(super) fn verdict(names: &[String], report: &Report, cut_short: &str) -> TestRun {
    let reported: Vec<Option<&Reported>> = names.iter().map(|name| report.tests.get(name)).collect();
    let failed = reported.iter().flatten().find_map(|outcome| match outcome {
        Reported::Failed(message) => Some(message.clone()),
        _ => None,
    });
    if let Some(message) = failed {
        return TestRun::Failed { message };
    }
    if reported.contains(&None) {
        return if report.finished {
            TestRun::NotRun { message: "cargo test ran it but reported nothing of it".to_string() }
        } else {
            TestRun::Failed { message: cut_short.to_string() }
        };
    }
    let ignored = reported.iter().flatten().find_map(|outcome| match outcome {
        Reported::Ignored(reason) => Some(reason.clone().unwrap_or_else(|| IGNORED.to_string())),
        _ => None,
    });
    match ignored {
        Some(message) => TestRun::Skipped { message },
        None => TestRun::Passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run's output, as cargo 1.100's libtest prints it.
    const RUN: &str = "
running 6 tests
test tests::plain_ignored ... ignored
test tests::slow ... ignored, too slow
test tests::adds ... ok
test tests::errs ... FAILED
test tests::boom - should panic ... ok
test tests::subtracts ... FAILED

failures:

---- tests::errs stdout ----
Error: \"bad thing\"

---- tests::subtracts stdout ----
noise

thread 'tests::subtracts' (70017107) panicked at src/lib.rs:5:49:
assertion `left == right` failed: two minus one is not zero
  left: 1
 right: 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tests::errs
    tests::subtracts

test result: FAILED. 2 passed; 2 failed; 2 ignored; 0 measured; 1 filtered out; finished in 0.00s
";

    fn names(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    fn failed(message: &str) -> TestRun {
        TestRun::Failed { message: message.to_string() }
    }

    #[test]
    fn a_listing_names_its_tests() {
        let stdout = "tests::adds: test\ntests::rounds::test_case_1_a_quarter_off: test\nspeed: benchmark\n\n2 tests, 1 benchmark\n";
        assert_eq!(listed(stdout), names("tests::adds tests::rounds::test_case_1_a_quarter_off"));
    }

    #[test]
    fn each_test_of_a_run_is_read_with_its_own_failure() {
        let read = report(RUN);
        assert!(read.finished);
        assert_eq!(read.tests.len(), 6);
        assert_eq!(read.tests["tests::adds"], Reported::Passed);
        assert_eq!(read.tests["tests::boom"], Reported::Passed);
        assert_eq!(read.tests["tests::plain_ignored"], Reported::Ignored(None));
        assert_eq!(read.tests["tests::slow"], Reported::Ignored(Some("too slow".into())));
        assert_eq!(read.tests["tests::errs"], Reported::Failed("Error: \"bad thing\"".into()));
        let subtracts = "thread 'tests::subtracts' (70017107) panicked at src/lib.rs:5:49:\nassertion `left == right` failed: two minus one is not zero\n  left: 1\n right: 0";
        assert_eq!(read.tests["tests::subtracts"], Reported::Failed(subtracts.into()));
    }

    #[test]
    fn a_test_passes_when_every_case_of_it_passed() {
        let read = report(RUN);
        let verdict = |tests: &str| verdict(&names(tests), &read, "ended early");
        assert_eq!(verdict("tests::adds tests::boom"), TestRun::Passed);
        assert_eq!(verdict("tests::adds tests::errs"), failed("Error: \"bad thing\""));
        assert_eq!(verdict("tests::adds tests::slow"), TestRun::Skipped { message: "too slow".into() });
        assert_eq!(verdict("tests::plain_ignored"), TestRun::Skipped { message: IGNORED.into() });
        assert_eq!(verdict("tests::errs tests::slow"), failed("Error: \"bad thing\""));
        let unreported = TestRun::NotRun { message: "cargo test ran it but reported nothing of it".into() };
        assert_eq!(verdict("tests::gone"), unreported);
    }

    #[test]
    fn a_run_whose_program_ended_early_fails_what_it_did_not_report() {
        let read = report("running 2 tests\ntest tests::adds ... ok\n");
        assert!(!read.finished);
        assert_eq!(verdict(&names("tests::adds"), &read, "aborted"), TestRun::Passed);
        assert_eq!(verdict(&names("tests::adds tests::aborts"), &read, "aborted"), failed("aborted"));
    }

    #[test]
    fn a_run_that_ended_early_says_how_from_cargos_last_lines() {
        let stderr = "     Running tests/math.rs (target/debug/deps/math-1)\nerror: test failed, to rerun pass `--test math`\n\nCaused by:\n  process didn't exit successfully: `math-1 --exact tests::aborts` (signal: 6, SIGABRT: process abort signal)\n";
        assert_eq!(
            ended_early(stderr),
            "the test program ended before it reported every test:\nerror: test failed, to rerun pass `--test math`\nCaused by:\nprocess didn't exit successfully: `math-1 --exact tests::aborts` (signal: 6, SIGABRT: process abort signal)"
        );
    }

    #[test]
    fn a_build_that_failed_gives_its_first_error() {
        let stderr = "   Compiling exp v0.1.0 (/p)\nwarning: unused\n\nerror[E0308]: mismatched types\n --> tests/math.rs:2:22\n  |\n2 | fn broken() -> u32 { \"no\" }\n  |                      ^^^^ expected `u32`, found `&str`\n\nerror: could not compile `exp` (test \"math\") due to 1 previous error\n";
        let first = first_error(stderr);
        assert!(first.starts_with("error[E0308]: mismatched types\n --> tests/math.rs:2:22"));
        assert!(first.ends_with("expected `u32`, found `&str`"));
        assert_eq!(
            first_error("warning: x\nerror: no matching package named `gone`"),
            "error: no matching package named `gone`"
        );
        assert_eq!(first_error("nothing useful\n"), "nothing useful");
    }
}
