//! Generated programs that serve items (fact expressions, the rows of a table, what a
//! surface offers) one by one: `harness::serve` is the part of one that is the same
//! every time, and a main the session writes is the rest.

use super::Runner;
use super::generated::{Dependencies, general, line_in, program_name};
use crate::toolchain::cargo::Problem;
use crate::toolchain::{Command, Outcome, run};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a generated program may take to start, before its own code runs. A block's time
/// limit and a fact or cases program's items are counted from then; a system that takes
/// longer to start a program is not loaded but broken.
pub(crate) const PROGRAM_STARTUP: Duration = Duration::from_secs(10);

/// A generated program as far as it could be built.
#[derive(Clone, Default)]
pub(super) struct Built {
    pub(super) binary: Option<PathBuf>,
    /// The pieces the compiler complained of, with the complaint, by index.
    pub(super) dropped: HashMap<usize, String>,
    /// Why there is no program at all, when it is no piece's fault.
    pub(super) failure: Option<String>,
}

/// A program on its way to being built: what is known of it, and the pieces still in it.
struct Building {
    built: Built,
    live: Vec<bool>,
}

impl Building {
    /// Reads what a build says of the program, and says whether it is settled: it was
    /// built, it cannot be, or every piece of it was taken out. It is not while the
    /// build took pieces out and some are left; it is then built again without them.
    fn settle(&mut self, name: &str, rendered: &Rendered, problems: &[Problem]) -> bool {
        let before = self.built.dropped.len();
        for problem in problems {
            if let Some(piece) = line_in(problem, name).and_then(|line| rendered.lines.get(&line)) {
                self.live[*piece] = false;
                self.built.dropped.entry(*piece).or_insert_with(|| problem.message.clone());
            }
        }
        if self.built.dropped.len() > before {
            return !self.live.contains(&true);
        }
        // A problem of another program of the same build is not this one's.
        let stops =
            problems.iter().find(|problem| problem.target == name || !problem.target.starts_with("p_"));
        self.built.failure = stops.map(general);
        true
    }
}

/// A program's source, and the line each of its pieces of attested code is on.
pub(super) struct Rendered {
    pub(super) source: String,
    pub(super) lines: HashMap<usize, usize>,
}

/// Writes a program's main for those of its pieces that are still in it.
pub(super) type Render<'a> = &'a dyn Fn(&[bool]) -> Rendered;

/// A program to build: what it is built with, how many pieces it has, and its `render`.
pub(super) struct Program<'a> {
    pub(super) dependencies: Dependencies,
    pub(super) pieces: usize,
    pub(super) render: Render<'a>,
}

/// One line of a program's results.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Line {
    pub(super) value: Value,
    pub(super) panic: Option<String>,
    pub(super) timeout: bool,
    pub(super) failed: Option<String>,
    /// Why an item has no answer: its program ended first.
    pub(super) lost: Option<String>,
}

impl Runner {
    /// Writes and builds a program whose main is made of pieces of attested code, each
    /// on a line of its own. `render` writes the main for the pieces still in it. A
    /// piece the compiler complains of is taken out, with the complaint, and the rest is
    /// built again: one wrong fact does not cost a page its other facts.
    pub(super) fn build_program(&self, pieces: usize, render: Render) -> Built {
        self.build_programs(&[Program { dependencies: Dependencies::Project, pieces, render }]).remove(0)
    }

    /// Builds several such programs together: one run of cargo for each package builds
    /// all of its programs that are not settled yet.
    pub(super) fn build_programs(&self, programs: &[Program]) -> Vec<Built> {
        let mut states: Vec<Building> = programs
            .iter()
            .map(|program| Building { built: Built::default(), live: vec![true; program.pieces] })
            .collect();
        let mut open: Vec<usize> = (0..programs.len()).collect();
        while !open.is_empty() {
            let rendered: Vec<(usize, String, Rendered)> = open
                .iter()
                .map(|&k| {
                    let rendered = (programs[k].render)(&states[k].live);
                    // The packages share a target directory, where a program's name is its file's.
                    let named = format!("{:?}{}", programs[k].dependencies, rendered.source);
                    (k, program_name("p", &named), rendered)
                })
                .collect();
            let sources: Vec<(Dependencies, String, String)> = rendered
                .iter()
                .map(|(k, name, rendered)| (programs[*k].dependencies, name.clone(), rendered.source.clone()))
                .collect();
            let problems = self.build_each(&sources);
            open.clear();
            for (k, name, source) in rendered {
                let problems = problems.get(&programs[k].dependencies).map_or(&[][..], Vec::as_slice);
                if !states[k].settle(&name, &source, problems) {
                    open.push(k);
                } else if states[k].built.failure.is_none()
                    && (states[k].live.is_empty() || states[k].live.contains(&true))
                {
                    states[k].built.binary = Some(self.binary(&name));
                }
            }
        }
        states.into_iter().map(|state| state.built).collect()
    }

    /// Runs a program until each of its `count` items has an answer, and returns the
    /// answers in order, or why the program served none. A program ends at an item that
    /// runs out of time or ends the process, and is started again at the item after it.
    pub(super) fn serve(
        &self,
        binary: &Path,
        cases: Option<Value>,
        count: usize,
    ) -> Result<Vec<Line>, String> {
        let dir = self.scratch(binary)?;
        let snapshots = &self.snapshots;
        let job = json!({ "timeoutMs": self.limits.timeout_ms, "snapshots": { "dir": snapshots.dir, "data": snapshots.data }, "manifests": self.manifests, "texts": self.texts, "cases": cases });
        let (job_file, results) = (dir.join("job.json"), dir.join("results"));
        std::fs::write(&job_file, job.to_string())
            .map_err(|err| format!("cannot write {}: {err}", job_file.display()))?;
        let mut answers: Vec<Line> = Vec::new();
        while answers.len() < count {
            let args = [&job_file, &results]
                .map(|path| path.to_string_lossy().into_owned())
                .into_iter()
                .chain([answers.len().to_string()]);
            let backstop = self.timeout() * (count - answers.len()) as u32 + PROGRAM_STARTUP;
            let command = Command {
                program: binary.to_path_buf(),
                args: args.collect(),
                dir: self.analysis.root.clone(),
                timeout: Some(backstop),
                ..Command::default()
            };
            let outcome = self.running(|| run(&command));
            let before = answers.len();
            answers = read_lines(&results)?;
            if answers.len() < count && (answers.len() == before || !answers[answers.len() - 1].timeout) {
                record_lost(&results, &outcome)?;
                answers = read_lines(&results)?;
            }
        }
        // The job's directory is the session's own and nothing reads it again.
        let _ = std::fs::remove_dir_all(&dir);
        Ok(answers)
    }
}

/// Adds to a program's results that the item it was at has no answer, since the
/// program ended first, so that the next start goes on after it.
fn record_lost(results: &Path, outcome: &Outcome) -> Result<(), String> {
    let written = json!({ "lost": lost(outcome) });
    let cannot = |err: std::io::Error| format!("cannot write {}: {err}", results.display());
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(results).map_err(cannot)?;
    std::io::Write::write_all(&mut file, format!("{written}\n").as_bytes()).map_err(cannot)
}

/// How a program ended that did not answer an item.
fn lost(outcome: &Outcome) -> String {
    match (outcome.code, outcome.stderr.lines().rev().find(|line| !line.trim().is_empty())) {
        (None, _) => "the program did not answer and was stopped".to_string(),
        (Some(code), Some(said)) => format!("exited with status {code}: {}", said.trim()),
        (Some(code), None) => format!("exited with status {code}"),
    }
}

/// Reads a program's results so far; an error is why the program served nothing. A
/// program that has answered nothing yet has written no results. A line cut short, by a
/// program ended while it wrote, is no answer: its item is then lost.
fn read_lines(results: &Path) -> Result<Vec<Line>, String> {
    let text = match std::fs::read_to_string(results) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(format!("cannot read {}: {err}", results.display())),
    };
    let lines: Vec<Line> = text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
    match lines.iter().find_map(|line| line.failed.clone()) {
        Some(failed) => Err(failed),
        None => Ok(lines),
    }
}
