//! `mutate`: how strong each proof is. Every proof that passes is run again against changes
//! to what it reads, one change at a time; a proof that some change makes fail is killed,
//! one that no change makes fail survived, and one nothing could be changed for is not
//! mutated. What a proof reads of the code is changed by the adapter's edits (`mutants`), in
//! copies of the project; the files facts read and the cells of tables are changed by the
//! core, in the project's own session.

mod code;
mod code_jobs;
mod effect;
mod in_session;
mod interrupt;
mod proofs;
mod scratch;
mod sibling;
pub mod text;
mod verdict;
mod workers;

use std::collections::{HashMap, HashSet};

use crate::api::attestation::Attestation;
use crate::api::errors::{Error, Result};
use crate::files::project_files::ProjectFiles;
use crate::project::check::{Network, SymbolRef, Symbols, results_and_symbols};
use crate::project::state::State;
use crate::report::{Outcome, Status};
use code_jobs::CodePlan;
use effect::{Effect, effect_of, effect_of_failing};
use in_session::{InSession, cell_trials, read_trials};
use proofs::{Proof, ProofKey, Runner, baseline, failed, outcomes_of, proofs_on, runner};
pub use verdict::Mutation;
use verdict::{Trial, Verdict, judged, verdict_of};
use workers::{JobRun, Place, run_jobs, run_unchanged};

/// The most edits of one symbol the adapter is asked for: each runs the proofs that read
/// the symbol once more, with an adapter of its own.
pub const MUTANTS_PER_SYMBOL: usize = 8;
/// The most changes of what one call of a fact that reads a file gives: each is one more
/// evaluation in the project's own session, so many cost little.
pub const MUTANTS_PER_READ: usize = 64;

/// The proofs on the `selected` pages, each judged by the mutants of what it reads.
pub fn run_mutate(mut state: State, network: Network, selected: &HashSet<String>) -> Result<Mutation> {
    let reads = state.fact_files.add_mutants(MUTANTS_PER_READ);
    let proofs = proofs_on(&state.attestations, selected);
    let run: Vec<Proof> =
        proofs.iter().copied().filter(|p| runner(p.attestation) == Runner::Adapter).collect();
    let mut changed = read_trials(&run, &reads);
    changed.extend(cell_trials(&run));
    let session = run_session(&state, network, &run, &changed)?;
    let code = plan_code(&state, &session)?;
    state.adapter.finish_session()?;
    let own = state.files.list();
    let place = Place {
        project: state.files.root(),
        own: &own,
        launches: &state.launches,
        now: state.now,
        network,
        workers: state.config.workers as usize,
    };
    let CodeRun { plan: code, trials: code_trials, stillborn } = run_code(&place, code)?;
    let mut trials = session.trials;
    // A change made in the session is one mutant of one proof; a change of code is one
    // mutant, however many proofs read what it changes.
    let mutants = trials.values().map(Vec::len).sum::<usize>() + code.jobs.len();
    for (key, trial) in code_trials {
        trials.entry(key).or_default().push(trial);
    }
    let verdicts = proofs.iter().map(|p| (p.key(), verdict(p, &session.baselines, &trials, &code))).collect();
    Ok(judged(&proofs, verdicts, mutants, stillborn))
}

/// What the project's own session gave: each proof's status as it is, the trials of the
/// changes made in it, and the symbols each passing proof reads.
struct Session {
    baselines: HashMap<ProofKey, Status>,
    trials: HashMap<ProofKey, Vec<Trial>>,
    reading: Vec<(ProofKey, Vec<SymbolRef>)>,
}

/// Run the proofs as they are, and every change made in the session beside them.
fn run_session(state: &State, network: Network, run: &[Proof], changed: &[InSession]) -> Result<Session> {
    let mut chosen: Vec<&Attestation> = Vec::new();
    for proof in run {
        if !chosen.iter().any(|a| a.id == proof.attestation.id) {
            chosen.push(proof.attestation);
        }
    }
    chosen.extend(changed.iter().map(|c| &c.attestation));
    let (results, symbols) = results_and_symbols(state, network, &chosen)?;
    let by_id: HashMap<&str, &[Outcome]> =
        chosen.iter().map(|a| a.id.as_str()).zip(results.iter().map(Vec::as_slice)).collect();
    let baselines: HashMap<ProofKey, Status> = run
        .iter()
        .map(|p| (p.key(), baseline(&outcomes_of(&p.key(), by_id[p.attestation.id.as_str()]))))
        .collect();
    let passes = |key: &ProofKey| baselines.get(key) == Some(&Status::Pass);
    let mut trials: HashMap<ProofKey, Vec<Trial>> = HashMap::new();
    for change in changed.iter().filter(|c| passes(&c.proof)) {
        let failing = failed(&outcomes_of(&change.proof, by_id[change.attestation.id.as_str()]));
        let effect = effect_of_failing(failing);
        let trial = Trial { description: change.description.clone(), effect, column: change.column.clone() };
        trials.entry(change.proof.clone()).or_default().push(trial);
    }
    let reading =
        run.iter().filter(|p| passes(&p.key())).map(|p| (p.key(), symbols_of(&symbols, p))).collect();
    Ok(Session { baselines, trials, reading })
}

fn symbols_of(symbols: &Symbols, proof: &Proof) -> Vec<SymbolRef> {
    symbols.get(&proof.attestation.id).cloned().unwrap_or_default()
}

/// The jobs for the edits the adapter answers for the symbols the passing proofs read.
fn plan_code(state: &State, session: &Session) -> Result<CodePlan> {
    let symbols = code_jobs::symbols_read(&session.reading);
    if symbols.is_empty() {
        return Ok(CodePlan::default());
    }
    if !state.adapter.can(code::CAPABILITY) {
        let why = format!("the {} adapter does not change code", state.adapter.name());
        return Ok(CodePlan { whys: code_jobs::explain_all(&session.reading, &why), ..CodePlan::default() });
    }
    let answers =
        code::read_answer(code::ask(&state.adapter, &symbols, MUTANTS_PER_SYMBOL).wait()?, symbols.len())?;
    let own: HashSet<String> = state.files.list().into_iter().collect();
    code_jobs::plan(&state.files, &own, &session.reading, &symbols, answers)
}

/// What running the mutants of code gave.
struct CodeRun {
    /// The plan as run: without the jobs of proofs that fail in a copy as it is.
    plan: CodePlan,
    /// A trial for each proof of each job.
    trials: Vec<(ProofKey, Trial)>,
    /// How many jobs left the code unable to build or load, for every proof they ran.
    stillborn: usize,
}

/// Run the plan's jobs, once the proofs they are for are known to pass in a copy of the
/// project as it is.
fn run_code(place: &Place, plan: CodePlan) -> Result<CodeRun> {
    if plan.jobs.is_empty() {
        return Ok(CodeRun { plan, trials: Vec::new(), stillborn: 0 });
    }
    let keys: Vec<ProofKey> = plan.described.iter().flat_map(|(_, keys)| keys.iter().cloned()).collect();
    let ids: Vec<String> =
        keys.iter().map(|key| key.attestation.clone()).collect::<HashSet<_>>().into_iter().collect();
    sibling::clear_stale(place.project).map_err(|e| {
        Error::user(format!(
            "cannot remove the copies a stopped run left beside {}: {e}",
            place.project.display()
        ))
    })?;
    let unchanged = run_unchanged(place, &ids)?;
    let failing: HashSet<ProofKey> =
        keys.into_iter().filter(|key| effect_in(&unchanged, key, "") != Effect::Passed).collect();
    let plan = plan.without(&failing, "it does not pass in a copy of the project, beside it");
    let runs = run_jobs(place, &plan.jobs)?;
    let (mut trials, mut stillborn) = (Vec::new(), 0);
    for (((description, keys), run), job) in plan.described.iter().zip(&runs).zip(&plan.jobs) {
        let effects: Vec<Effect> = keys.iter().map(|key| effect_in(run, key, &job.file)).collect();
        stillborn += usize::from(effects.iter().all(|e| *e == Effect::Stillborn));
        for (key, effect) in keys.iter().zip(effects) {
            trials.push((key.clone(), Trial { description: description.clone(), effect, column: None }));
        }
    }
    Ok(CodeRun { plan, trials, stillborn })
}

/// What a change to `changed` did to the proof `key` in `run`. A project that cannot be
/// loaded or opened with the change did not build; a proof the run did not hold passed.
fn effect_in(run: &JobRun, key: &ProofKey, changed: &str) -> Effect {
    match run {
        JobRun::Broken(_) => Effect::Stillborn,
        JobRun::Ran(outcomes) => outcomes
            .get(&key.attestation)
            .map_or(Effect::Passed, |o| effect_of(&outcomes_of(key, o), changed)),
    }
}

/// The verdict on one proof.
fn verdict(
    proof: &Proof,
    baselines: &HashMap<ProofKey, Status>,
    trials: &HashMap<ProofKey, Vec<Trial>>,
    code: &CodePlan,
) -> Verdict {
    let key = proof.key();
    let not_mutated = |why: &str| Verdict::NotMutated { why: why.to_string() };
    match runner(proof.attestation) {
        Runner::Citation => {
            return not_mutated("a citation: docattest reads the test file and does not run the test");
        }
        Runner::Core => return not_mutated("a block the core runs itself: what it reads is not known"),
        Runner::Adapter => {}
    }
    match baselines.get(&key) {
        Some(Status::Pass) => {}
        Some(Status::Skipped) => return not_mutated("it was skipped"),
        _ => return not_mutated("it does not pass as it is"),
    }
    if let Some(trials) = trials.get(&key).filter(|t| !t.is_empty()) {
        return verdict_of(trials);
    }
    match code.why_not(&key) {
        Some(why) => not_mutated(&why),
        None => not_mutated("it reads no symbol of the project and no file that could be changed"),
    }
}
#[cfg(test)]
mod tests;
