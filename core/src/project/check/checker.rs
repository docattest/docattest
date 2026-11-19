//! The checker: runs a set of attestations through the adapter, and reads the outcomes and
//! observations it answers with.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::cited_runs::{CitedRuns, CitedTests, SentRuns};
use super::display::{Displayed, send_display};
use super::sample::check_sample;
use super::{Network, Symbols, symbol_names};
use crate::adapter::Pending;
use crate::api::attestation::{Attestation, Body, Kind, Mode};
use crate::api::errors::{Error, Result};
use crate::attestations::judge::judge_citation;
use crate::compare::{Observation, compare_fact};
use crate::console::block::{ConsolePlace, run_console};
use crate::project::state::State;
use crate::report::{Outcome, Status};

/// Requests sent before compiling.
pub(super) struct Started {
    evaluated: Pending,
    displayed: Option<Pending>,
    listings: Vec<(String, Pending)>,
    tables: Vec<Pending>,
}

/// Runs a set of attestations through the adapter and collects their results. Work
/// overlaps; results are kept per attestation and flattened in plan order, so the report
/// never depends on scheduling.
pub(super) struct Checker<'s> {
    state: &'s State,
    /// Whether examples tagged `network` may reach it, here and on a sample page.
    network: Network,
    pub(super) symbols: Symbols,
    by_attestation: HashMap<String, Vec<Outcome>>,
    cited: CitedTests,
    /// The run of the tests that citations name, when `cited` asks for one.
    runs: Option<SentRuns>,
    /// How many citations had their test run, whatever it gave.
    pub(super) cited_ran: usize,
}

impl<'s> Checker<'s> {
    pub(super) fn new(state: &'s State, network: Network, cited: CitedTests) -> Self {
        Checker {
            state,
            network,
            symbols: Symbols::new(),
            by_attestation: HashMap::new(),
            cited,
            runs: None,
            cited_ran: 0,
        }
    }

    /// Ask the adapter to compile the examples; nothing is asked when there are none, so an
    /// adapter never builds a program for nothing.
    pub(super) fn send_compile(&self, examples: &[&Attestation]) -> Option<Pending> {
        let list: Vec<Value> = examples.iter().map(|a| a.to_json()).collect();
        (!list.is_empty()).then(|| self.state.adapter.send("compile", json!({ "examples": list })))
    }

    /// Send what needs no compiling: facts, citations, tables.
    pub(super) fn start(&self, attestations: &[&Attestation]) -> Started {
        let adapter = &self.state.adapter;
        let of = |kind: Kind| -> Vec<&Attestation> {
            attestations.iter().copied().filter(|a| a.ok() && a.kind() == kind).collect()
        };
        let ok = |kind: Kind| -> Vec<Value> { of(kind).iter().map(|a| a.to_json()).collect() };
        let evaluated = adapter.send("evaluate", json!({ "facts": ok(Kind::Fact) }));
        let displayed = send_display(adapter, &of(Kind::Fact));
        let mut listings: Vec<(String, Pending)> = Vec::new();
        for c in attestations.iter().filter(|a| a.ok()) {
            if let Body::Test(t) = &c.body
                && !listings.iter().any(|(f, _)| *f == t.file)
            {
                listings.push((t.file.clone(), adapter.send("tests", json!({ "file": t.file }))));
            }
        }
        let tables = ok(Kind::Cases)
            .into_iter()
            .map(|c| adapter.send("run", json!({ "examples": [], "cases": [c] })))
            .collect();
        Started { evaluated, displayed, listings, tables }
    }

    /// The results of each attestation, in a list of its own, in the order given.
    pub(super) fn check(
        &mut self,
        attestations: &[&Attestation],
        started: Started,
        compiled: Option<Pending>,
    ) -> Result<Vec<Vec<Outcome>>> {
        let adapter = &self.state.adapter;
        let ok = self.emit_errors(attestations)?;
        let of =
            |kind: Kind| -> Vec<&Attestation> { ok.iter().copied().filter(|a| a.kind() == kind).collect() };
        let examples: Vec<&Attestation> =
            ok.iter().copied().filter(|a| matches!(a.body, Body::Example(_))).collect();
        let (facts, cases, citations) = (of(Kind::Fact), of(Kind::Cases), of(Kind::Test));
        let Started { evaluated, displayed, listings, tables } = started;
        let runnable = self.compile(&examples, compiled)?;
        let runs: Vec<Pending> = runnable
            .iter()
            .map(|e| adapter.send("run", json!({ "examples": [e.to_json()], "cases": [] })))
            .collect();
        // The adapter works on what it was sent while the core runs its own blocks.
        self.run_by_core(&ok)?;
        self.evaluate(&facts, evaluated, displayed)?;
        self.cite(&citations, listings)?;
        self.emit_runs(&runnable, runs)?;
        self.emit_runs(&cases, tables)?;
        self.emit_cited_runs()?;
        Ok(attestations.iter().map(|a| self.by_attestation.remove(&a.id).unwrap_or_default()).collect())
    }

    /// Emits the attestations planning found in error; returns the others.
    fn emit_errors<'a>(&mut self, attestations: &[&'a Attestation]) -> Result<Vec<&'a Attestation>> {
        let mut ok = Vec::new();
        for a in attestations {
            match a.problem() {
                Some(error) => {
                    self.emit(a, vec![Outcome::new(&a.page, a.line, a.kind(), Status::Error, error)])?
                }
                None => ok.push(*a),
            }
        }
        Ok(ok)
    }

    /// Emits what each run answered, in the order of the attestations.
    fn emit_runs(&mut self, attestations: &[&Attestation], runs: Vec<Pending>) -> Result<()> {
        for (a, run) in attestations.iter().zip(runs) {
            let results = outcomes(run.wait()?)?;
            self.emit(a, results)?;
        }
        Ok(())
    }

    /// Emit compile verdicts; returns the `check` examples that compiled and should run.
    fn compile<'a>(
        &mut self,
        examples: &[&'a Attestation],
        compiled: Option<Pending>,
    ) -> Result<Vec<&'a Attestation>> {
        let diagnostics = match compiled {
            Some(pending) => pending.wait()?.as_array().cloned().unwrap_or_default(),
            None => Vec::new(),
        };
        let mut runnable = Vec::new();
        for e in examples {
            let mine = diagnostics_of(&diagnostics, &e.id)?;
            let Body::Example(example) = &e.body else { continue };
            if !mine.is_empty() {
                let mut diagnosed =
                    Outcome::new(&e.page, e.line, Kind::Example, Status::Fail, &mine.join("; "));
                diagnosed.compile_diagnostics = true;
                self.emit(e, vec![diagnosed])?;
                continue;
            }
            if example.mode == Mode::Compile {
                self.emit(e, vec![Outcome::new(&e.page, e.line, Kind::Example, Status::Pass, "compiles")])?;
            } else {
                runnable.push(*e);
            }
        }
        Ok(runnable)
    }

    /// Run the blocks among `attestations` that the core runs itself, one after another, in
    /// plan order.
    fn run_by_core(&mut self, attestations: &[&Attestation]) -> Result<()> {
        let root = self.state.files.root();
        let adapters = self.state.launches.variables();
        let place = ConsolePlace { root, timeout_ms: self.state.config.timeout_ms, adapters };
        for block in attestations {
            let outcome = match &block.body {
                Body::Console(console) => run_console(block, console, &place),
                Body::Sample(sample) => check_sample(block, sample, self.network, self.state)?,
                Body::Example(_) | Body::Fact(_) | Body::Cases(_) | Body::Test(_) => continue,
            };
            self.emit(block, vec![outcome])?;
        }
        Ok(())
    }

    fn evaluate(
        &mut self,
        facts: &[&Attestation],
        evaluated: Pending,
        displayed: Option<Pending>,
    ) -> Result<()> {
        let values = evaluated.wait()?;
        let displayed = Displayed::wait(displayed, facts)?;
        for fact in facts {
            let Body::Fact(f) = &fact.body else { continue };
            let value = values.get(&fact.id);
            let (Some(value), Ok(target)) = (value, &f.target) else {
                return Err(Error::internal(format!("fact {} was not evaluated", fact.id)));
            };
            if value.get("ok") != Some(&Value::Bool(true)) {
                let message = said(value.get("message"), "evaluate answered a failure without a message")?;
                self.emit(
                    fact,
                    vec![Outcome::new(&fact.page, fact.line, Kind::Fact, Status::Error, &message)],
                )?;
                continue;
            }
            let observed = observation(value.get("value"))?;
            let c = compare_fact(&f.expression, f.ordered, displayed.target(fact, target), &observed);
            let outcome = Outcome::new(&fact.page, fact.line, Kind::Fact, c.status, &c.message)
                .with_comparison(&c.expected, &c.actual);
            self.emit(fact, vec![outcome])?;
        }
        Ok(())
    }

    // Citations are judged from the cited file's syntax. With `--run-cited`, the tests of
    // the ones that hold are sent to run, and those citations wait for their runs.
    fn cite(&mut self, citations: &[&Attestation], listings: Vec<(String, Pending)>) -> Result<()> {
        let mut answers: HashMap<String, Value> = HashMap::new();
        let mut pending: HashMap<String, Pending> = listings.into_iter().collect();
        let mut runs = CitedRuns::default();
        for c in citations {
            let Body::Test(t) = &c.body else { continue };
            let listing = listing_of(&t.file, &mut answers, &mut pending)?;
            let judged = judge_citation(t, listing, &self.state.adapter.vocabulary)?;
            let outcome = match judged.found {
                Some(found) if self.cited == CitedTests::Run => runs.add(c, &t.file, found),
                _ => Some(Outcome::new(&c.page, c.line, Kind::Test, judged.status, &judged.message)),
            };
            if let Some(outcome) = outcome {
                self.emit(c, vec![outcome])?;
            }
        }
        self.runs = runs.send(&self.state.adapter);
        Ok(())
    }

    /// Emits what running the cited tests made of their citations.
    fn emit_cited_runs(&mut self) -> Result<()> {
        let Some(runs) = self.runs.take() else { return Ok(()) };
        for run in runs.outcomes()? {
            self.cited_ran += usize::from(run.ran);
            self.emit(&run.citation, vec![run.outcome])?;
        }
        Ok(())
    }

    fn emit(&mut self, attestation: &Attestation, results: Vec<Outcome>) -> Result<()> {
        let names = symbol_names(self.symbols.get(&attestation.id).map_or(&[][..], Vec::as_slice));
        let mut located = Vec::with_capacity(results.len());
        for mut r in results {
            r.symbols.clone_from(&names);
            located.push(r);
        }
        self.by_attestation.entry(attestation.id.clone()).or_default().extend(located);
        Ok(())
    }
}

/// What `tests` answered for `file`, waited for the first time a citation asks; null when it
/// was never asked.
fn listing_of<'a>(
    file: &str,
    answers: &'a mut HashMap<String, Value>,
    pending: &mut HashMap<String, Pending>,
) -> Result<&'a Value> {
    if !answers.contains_key(file) {
        let listing = match pending.remove(file) {
            Some(p) => p.wait()?,
            None => Value::Null,
        };
        answers.insert(file.to_string(), listing);
    }
    Ok(answers.get(file).unwrap_or(&Value::Null))
}

/// The messages of the compile diagnostics for one example, each with its block line.
fn diagnostics_of(diagnostics: &[Value], id: &str) -> Result<Vec<String>> {
    diagnostics
        .iter()
        .filter(|d| d.get("id").and_then(Value::as_str) == Some(id))
        .map(|d| {
            let message = said(d.get("message"), "compile answered a diagnostic without a message")?;
            Ok(format!("{message} (block line {})", diagnostic_line(d)?))
        })
        .collect()
}

/// A diagnostic's line in its block, as the text of the number the adapter sent; 1 when it
/// has none.
fn diagnostic_line(diagnostic: &Value) -> Result<String> {
    match diagnostic.get("line") {
        None | Some(Value::Null) => Ok("1".to_string()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        Some(_) => Err(Error::internal("compile answered a line that is not a number")),
    }
}

fn outcomes(value: Value) -> Result<Vec<Outcome>> {
    let Value::Array(items) = value else {
        return Err(Error::internal("run did not answer a list"));
    };
    items.into_iter().map(Outcome::from_json).collect()
}

/// A string the protocol requires of an answer. One that is missing is the adapter's bug:
/// the core has no word of its own to put in its place.
fn said(value: Option<&Value>, otherwise: &str) -> Result<String> {
    value.and_then(Value::as_str).map(str::to_string).ok_or_else(|| Error::internal(otherwise))
}

fn observation(value: Option<&Value>) -> Result<Observation> {
    let field = |key: &str| value.and_then(|v| v.get(key));
    let items = match field("items") {
        None => None,
        Some(items) => Some(
            items
                .as_array()
                .and_then(|a| a.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>())
                .ok_or_else(|| Error::internal("evaluate answered items that are not strings"))?,
        ),
    };
    Ok(Observation {
        text: said(field("text"), "evaluate answered a value without its text")?,
        items,
        description: said(field("description"), "evaluate answered a value without a description")?,
        is_text: field("kind").and_then(Value::as_str) == Some("text"),
    })
}
