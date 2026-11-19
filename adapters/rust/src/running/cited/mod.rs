//! The project's own tests that citations name, run by `cargo test` in the project: which
//! run selects each, and which of the tests it lists are the cited one's ([`selection`]),
//! and what libtest printed ([`report`]).
//!
//! The tests of one target run together, in two runs of `cargo test`: one that builds the
//! target and lists its tests, and one that runs exactly the names the citations select
//! from that listing. The listing is what makes the selection exact: a macro's names for
//! the cases of a test are read from it rather than guessed.

mod report;
mod selection;

use super::Runner;
use crate::analysis::tests::Printed;
use crate::project::Target;
use crate::protocol::{CitedTest, TestRun};
use crate::toolchain::cargo::{self, not_run};
use crate::toolchain::{Command, Outcome, run};
use report::Report;
use selection::Modules;
use std::path::Path;
use std::time::Duration;

/// A cited test, as far as its run is decided before anything runs.
struct Wanted<'a> {
    /// The module path of the test's file in its target.
    module: &'a [String],
    printed: Printed,
    /// The test's path as a message names it.
    shown: String,
}

/// The cited tests of one target, each with its place among the tests asked about.
type Group<'a> = (&'a Target, Vec<(usize, Wanted<'a>)>);

impl Runner {
    /// Answers `runTests`: how each cited test ran. `timeout` bounds each run of cargo,
    /// and as many targets run at once as the session has workers.
    pub(crate) fn run_tests(&self, tests: &[CitedTest], timeout: Duration) -> Vec<TestRun> {
        if let Some(error) = &self.analysis.project.error {
            return tests.iter().map(|_| TestRun::NotRun { message: error.clone() }).collect();
        }
        let read = |path: &Path| std::fs::read_to_string(path).ok();
        let modules = Modules::read(&self.analysis.project.targets, &read);
        let mut answers: Vec<Option<TestRun>> = vec![None; tests.len()];
        let mut groups: Vec<Group> = Vec::new();
        for (k, test) in tests.iter().enumerate() {
            match self.wanted(&modules, test) {
                Ok((target, wanted)) => match groups.iter_mut().find(|(by, _)| std::ptr::eq(*by, target)) {
                    Some((_, members)) => members.push((k, wanted)),
                    None => groups.push((target, vec![(k, wanted)])),
                },
                Err(message) => answers[k] = Some(TestRun::NotRun { message }),
            }
        }
        std::thread::scope(|scope| {
            let runs: Vec<_> =
                groups.iter().map(|group| scope.spawn(|| self.run_target(group, timeout))).collect();
            for (handle, (_, members)) in runs.into_iter().zip(&groups) {
                let ran = handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic));
                for ((k, _), outcome) in members.iter().zip(ran) {
                    answers[*k] = Some(outcome);
                }
            }
        });
        answers.into_iter().map(|answer| answer.expect("every cited test is answered")).collect()
    }

    /// The target that runs a cited test, and how cargo test prints it.
    fn wanted<'a>(
        &self,
        modules: &'a Modules<'a>,
        test: &CitedTest,
    ) -> Result<(&'a Target, Wanted<'a>), String> {
        let printed = self.analysis.printed(&test.file, &test.describes, &test.title)?;
        let (target, module) = modules.owner(&self.analysis.abs(&test.file), &test.file)?;
        let shown = [test.describes.as_slice(), std::slice::from_ref(&test.title)].concat().join(" > ");
        Ok((target, Wanted { module, printed, shown }))
    }

    /// Lists a target's tests and runs those the citations select, one outcome for each.
    fn run_target(&self, (target, members): &Group, timeout: Duration) -> Vec<TestRun> {
        let listed = match self.list(target, timeout) {
            Ok(listed) => listed,
            Err(outcome) => return vec![outcome; members.len()],
        };
        let selected: Vec<Vec<String>> = members
            .iter()
            .map(|(_, wanted)| selection::selected(&listed, wanted.module, &wanted.printed))
            .collect();
        let ran = self.run_selected(target, &distinct(&selected), timeout);
        let command = format!("cargo test {}", selection::selects(target).join(" "));
        let outcome = |(names, (_, wanted)): (&Vec<String>, &(usize, Wanted))| {
            if names.is_empty() {
                return TestRun::NotRun { message: format!("{command} lists no test {}", wanted.shown) };
            }
            match &ran {
                Ok((report, ended)) => report::verdict(names, report, ended),
                Err(outcome) => outcome.clone(),
            }
        };
        selected.iter().zip(members).map(outcome).collect()
    }

    /// Builds a target and lists its tests; a target that does not build fails every test
    /// of it with the first error.
    fn list(&self, target: &Target, timeout: Duration) -> Result<Vec<String>, TestRun> {
        let listed = self.cargo_test(selection::listing(target), timeout)?;
        if listed.code != Some(0) {
            return Err(TestRun::Failed { message: report::first_error(&listed.stderr) });
        }
        Ok(report::listed(&listed.stdout))
    }

    /// Runs exactly some tests of a target, by name: what libtest reported of them, and
    /// why its run ended early, if it did.
    fn run_selected(
        &self,
        target: &Target,
        names: &[String],
        timeout: Duration,
    ) -> Result<(Report, String), TestRun> {
        if names.is_empty() {
            return Ok((Report::default(), String::new()));
        }
        let ran = self.cargo_test(selection::running(target, names), timeout)?;
        Ok((report::report(&ran.stdout), report::ended_early(&ran.stderr)))
    }

    /// Runs `cargo` with some arguments in the project, when a worker is free, or says
    /// how every test of the run fared when it could not start or did not end in time.
    fn cargo_test(&self, args: Vec<String>, timeout: Duration) -> Result<Outcome, TestRun> {
        let command = Command {
            program: cargo::program(),
            args,
            dir: self.analysis.root.clone(),
            timeout: Some(timeout),
            // A backtrace would push a failure's own message out of the lines it is shown in.
            env: vec![("RUST_BACKTRACE".to_string(), "0".to_string())],
            ..Command::default()
        };
        let outcome = self.running(|| run(&command));
        if let Some(error) = outcome.error {
            return Err(TestRun::NotRun { message: not_run(&error) });
        }
        if outcome.stopped.is_some() {
            let message = format!("cargo test did not finish within {} s", timeout.as_secs_f64());
            return Err(TestRun::Failed { message });
        }
        Ok(outcome)
    }
}

/// The names of some selections, each once, in the order they were first selected.
fn distinct(selected: &[Vec<String>]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in selected.iter().flatten() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
}
