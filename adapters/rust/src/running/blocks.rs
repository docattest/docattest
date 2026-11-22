//! Examples: each is a program of the generated crate, built once and run by itself.

use super::Runner;
use super::generated::{Dependencies, HARNESS_CRATE, general, line_in, program_name};
use super::outcomes::Outcomes;
use super::programs::PROGRAM_STARTUP;
use crate::harness::started;
use crate::language::{NETWORK_SKIPPED, SHOULD_PANIC, blocks, output, text};
use crate::protocol::{Cases, Diagnostic, Example, Kind, Status, Verdict};
use crate::toolchain::cargo::{DOCS_CFG, Problem};
use crate::toolchain::{Command, Outcome, Start, Stopped, run};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// The exit status of a Rust program that panicked.
const PANICKED: i32 = 101;

/// One example as a program.
#[derive(Default)]
struct Block {
    /// Its program's name in the generated crate.
    name: String,
    /// How many lines of its program come before its first line.
    offset: usize,
    /// Why it does not compile: the line of the block, and the complaint.
    problems: Vec<(usize, String)>,
    /// Why it is not built at all: the line of the block, and the reason.
    refused: Option<(usize, String)>,
    /// Its program, when one was built.
    binary: Option<PathBuf>,
}

/// The examples of a session, each built once. One batch builds at a time; a second
/// waits and finds its blocks built.
#[derive(Default)]
pub(super) struct Blocks {
    by_id: Mutex<HashMap<String, Arc<Block>>>,
}

impl Runner {
    /// Builds examples ahead of the `compile` and `run` that will name them.
    pub(crate) fn build_ahead(&self, examples: &[Example]) {
        self.ensure(examples);
    }

    /// Builds, in one run of cargo, those of the examples that are not built yet, and
    /// returns every example's block by id.
    fn ensure(&self, examples: &[Example]) -> HashMap<String, Arc<Block>> {
        let mut by_id = self.blocks.by_id.lock().unwrap_or_else(|p| p.into_inner());
        let mut fresh: Vec<(String, Block)> = Vec::new();
        let mut sources: Vec<(String, String)> = Vec::new();
        let mut refusals: Vec<(String, Block)> = Vec::new();
        for example in examples.iter().filter(|e| e.error.is_none() && !by_id.contains_key(&e.id)) {
            if let Some(refused) = self.helper_refusal(&example.code) {
                refusals.push((example.id.clone(), Block { refused: Some(refused), ..Block::default() }));
                continue;
            }
            let program =
                blocks::program(&example.code, &format!("::{HARNESS_CRATE}::harness::started::signal();"));
            let name = program_name("e", &program.source);
            sources.push((name.clone(), program.source));
            fresh.push((example.id.clone(), Block { name, offset: program.offset, ..Block::default() }));
        }
        by_id.extend(refusals.into_iter().map(|(id, block)| (id, Arc::new(block))));
        if !fresh.is_empty() {
            let problems = self.build(Dependencies::Project, &sources);
            for (id, mut block) in fresh {
                self.settle(&mut block, &problems);
                by_id.insert(id, Arc::new(block));
            }
        }
        by_id.clone()
    }

    /// Why an example is refused before it is built, when it names a docs helper: the line
    /// of the block that first names one, and what to say. The example would compile, since
    /// every program is built with the docs helpers, but users build without them.
    fn helper_refusal(&self, code: &str) -> Option<(usize, String)> {
        let helper = self.analysis.docs_helpers_named(code).into_iter().next()?;
        let names = |line: &&str| text::identifiers(line).contains(&helper.name.as_str());
        let line = code.lines().position(|line| names(&line)).map_or(1, |k| k + 1);
        let why = format!(
            "uses {}, a docs helper of surface \"{}\": a page's example runs as users build the code, without #[cfg({DOCS_CFG})]",
            helper.name, helper.surface
        );
        Some((line, why))
    }

    /// Gives a block its program, or what the build complained of about it. A complaint
    /// about no block (the project's own code does not compile) is about every block
    /// that has no program.
    fn settle(&self, block: &mut Block, problems: &[Problem]) {
        for problem in problems.iter().filter(|problem| problem.target == block.name) {
            let line =
                line_in(problem, &block.name).map_or(1, |line| line.saturating_sub(block.offset).max(1));
            block.problems.push((line, problem.message.clone()));
        }
        let binary = self.binary(&block.name);
        if block.problems.is_empty() && binary.is_file() {
            block.binary = Some(binary);
        } else if block.problems.is_empty() {
            let about_none = problems.iter().find(|problem| !problem.target.starts_with("e_"));
            block
                .problems
                .push((1, about_none.map_or_else(|| "the block was not built".to_string(), general)));
        }
    }

    /// Answers `compile`: for each example, why it does not compile, if it does not.
    pub(crate) fn compile(&self, examples: &[Example]) -> Vec<Diagnostic> {
        let built = self.ensure(examples);
        let mut out = Vec::new();
        for example in examples {
            let block = built.get(&example.id);
            let refused = block.and_then(|block| block.refused.as_ref());
            let problems = block.map_or(&[][..], |block| &block.problems[..]);
            for (line, message) in refused.into_iter().chain(problems) {
                out.push(Diagnostic { id: example.id.clone(), message: message.clone(), line: *line });
            }
        }
        out
    }

    /// Answers `run`: a result for each example, in order, then for each row of each
    /// table. Everything runs at once, as far as the session's slots allow.
    pub(crate) fn run(&self, examples: &[Example], tables: &[Cases]) -> Vec<Verdict> {
        let built = self.ensure(examples);
        std::thread::scope(|scope| {
            let running: Vec<_> = examples
                .iter()
                .map(|example| {
                    let block = built.get(&example.id).cloned();
                    scope.spawn(move || {
                        vec![self.run_example(example, block).verdict(
                            &example.page,
                            example.line,
                            Kind::Example,
                        )]
                    })
                })
                .chain(tables.iter().map(|table| scope.spawn(move || self.run_cases(table))))
                .collect();
            running.into_iter().flat_map(|thread| thread.join().unwrap_or_default()).collect()
        })
    }

    fn run_example(&self, example: &Example, block: Option<Arc<Block>>) -> Outcomes {
        if example.network && !self.limits.network {
            return Outcomes::of(Status::Skipped, NETWORK_SKIPPED);
        }
        let Some(block) = block else { return Outcomes::fail("the block was not built") };
        if let Some((line, why)) = &block.refused {
            return Outcomes::fail(format!("{why} (block line {line})"));
        }
        if let Some((line, message)) = block.problems.first() {
            return Outcomes::fail(format!("does not compile: {message} (block line {line})"));
        }
        let Some(binary) = &block.binary else { return Outcomes::fail("the block was not built") };
        let dir = match self.scratch(binary) {
            Ok(dir) => dir,
            Err(why) => return Outcomes::fail(format!("could not be run: {why}")),
        };
        let command = Command {
            program: binary.clone(),
            args: Vec::new(),
            dir: self.analysis.root.clone(),
            timeout: Some(self.timeout()),
            start: Some(Start {
                variable: started::VARIABLE,
                signal: dir.join("started"),
                within: PROGRAM_STARTUP,
            }),
            ..Command::default()
        };
        let outcome = self.running(|| run(&command));
        // The directory is this run's own and nothing reads it again.
        let _ = std::fs::remove_dir_all(&dir);
        self.judge(example, &block, &outcome)
    }

    /// Judges how a block's program ended: it could not be run, it was stopped, or it ran to
    /// an exit.
    fn judge(&self, example: &Example, block: &Block, outcome: &Outcome) -> Outcomes {
        if let Some(error) = &outcome.error {
            return Outcomes::fail(format!("could not be run: {error}"));
        }
        match outcome.stopped {
            Some(Stopped::TimedOut) => Outcomes::fail(self.timed_out()),
            Some(Stopped::NotStarted) => Outcomes::fail(never_started()),
            None => exited(example, block, outcome),
        }
    }
}

/// Judges a block's program that ran to an exit, by its status and what the block says of it.
fn exited(example: &Example, block: &Block, outcome: &Outcome) -> Outcomes {
    let panics = example.tags.iter().any(|tag| tag == SHOULD_PANIC);
    match outcome.code {
        Some(PANICKED) if panics => Outcomes::pass("panicked, as it says it does"),
        Some(0) if panics => Outcomes::fail("ran to its end, and says it panics"),
        Some(0) => {
            if let Some(want) = output::expected(&example.code) {
                return match output::compare(&outcome.stdout, &want) {
                    output::OutputVerdict::Pass => Outcomes::pass("printed the output it states"),
                    output::OutputVerdict::Fail { message, expected, actual } => Outcomes {
                        status: Status::Fail,
                        message,
                        expected: Some(expected),
                        actual: Some(actual),
                    },
                };
            }
            if text::asserts(&example.code) {
                Outcomes::pass("all assertions held")
            } else {
                Outcomes::pass("ran to its end, but asserts nothing (weak)")
            }
        }
        code => crash(outcome, code, block),
    }
}

/// Why a program ended with a failure: its panic, both sides of a failed comparison, and
/// the line of the block the panic was at; else the last thing it wrote to its standard
/// error.
fn crash(outcome: &Outcome, code: Option<i32>, block: &Block) -> Outcomes {
    let lines: Vec<&str> = outcome.stderr.lines().collect();
    let Some(at) = lines.iter().position(|line| line.contains("panicked at ")) else {
        let said = lines
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map_or(String::new(), |line| format!(": {}", line.trim()));
        return Outcomes::fail(format!(
            "exited with status {}{said}",
            code.map_or("none".to_string(), |code| code.to_string())
        ));
    };
    let message = lines.get(at + 1).map_or("", |line| line.trim());
    let side = |name: &str| {
        lines[at..]
            .iter()
            .find_map(|line| line.trim_start().strip_prefix(name))
            .map(|value| value.trim().to_string())
    };
    let (left, right) = (side("left:"), side("right:"));
    let place = lines[at].split("panicked at ").nth(1).unwrap_or("");
    let line = place
        .strip_prefix(&format!("src/bin/{}.rs:", block.name))
        .and_then(|rest| rest.split(':').next()?.parse::<usize>().ok());
    let wherein = line
        .map_or(String::new(), |line| format!(" (block line {})", line.saturating_sub(block.offset).max(1)));
    let sides = match (&left, &right) {
        (Some(left), Some(right)) => format!(": expected {right}, got {left}"),
        _ => String::new(),
    };
    Outcomes {
        status: Status::Fail,
        message: format!("panicked: {message}{sides}{wherein}"),
        expected: right,
        actual: left,
    }
}

/// Why a block whose program never started fails: as the Go adapter says it, so a page
/// reads the same whichever language it documents.
fn never_started() -> String {
    format!("did not start within {} ms", PROGRAM_STARTUP.as_millis())
}

#[cfg(test)]
mod tests {
    use super::never_started;

    #[test]
    fn a_block_whose_program_never_starts_says_how_long_it_waited() {
        assert_eq!(never_started(), "did not start within 10000 ms");
    }
}
