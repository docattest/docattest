//! Running the mutants of code: each in a copy of the project ([`Scratch`]), with an adapter
//! of its own started on the copy, which runs only the proofs that read the changed symbol.
//! As many copies work at once as the project's `runtime.workers` allows; each takes the
//! next mutant when it is done with one, and puts its file back afterwards.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::scratch::Scratch;
use crate::api::errors::{Error, Result};
use crate::files::source::DiskFiles;
use crate::project::check::{Network, results_of};
use crate::project::launches::Launches;
use crate::project::state::load_state_beside;
use crate::report::Outcome;

/// One file changed, and the proofs (attestation ids) to run against the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeJob {
    pub file: String,
    /// The file's text with the change made.
    pub changed: String,
    pub proofs: Vec<String>,
}

/// What running the proofs of a job gave.
#[derive(Debug, Clone, PartialEq)]
pub enum JobRun {
    /// The outcomes of each proof that ran, by attestation id.
    Ran(HashMap<String, Vec<Outcome>>),
    /// The project could not be loaded or opened with the change: every proof fails.
    Broken(String),
}

/// What the runs share: the project and how its adapter is started.
pub struct Place<'a> {
    pub project: &'a Path,
    /// The project's own files, which the copies hold.
    pub own: &'a [String],
    pub launches: &'a Launches,
    pub now: f64,
    pub network: Network,
    /// How many copies may work at once.
    pub workers: usize,
}

/// The proofs `proofs` run in a copy as it is: which of them the copy runs as the project
/// does is found here, before any change is trusted to tell anything.
pub fn run_unchanged(place: &Place, proofs: &[String]) -> Result<JobRun> {
    let scratch = Scratch::beside(place.project, place.own, 0).map_err(|e| copy_failed(place.project, &e))?;
    run_proofs(place, scratch.root(), proofs, 1)
}

/// Each job's run, in the order of `jobs`.
pub fn run_jobs(place: &Place, jobs: &[CodeJob]) -> Result<Vec<JobRun>> {
    let copies = place.workers.clamp(1, jobs.len().max(1));
    // The adapter of each copy runs its proofs with a share of the workers, so that the
    // copies together use about as many as one check would.
    let each = (place.workers / copies).max(1);
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Option<JobRun>>> = Mutex::new(vec![None; jobs.len()]);
    let (next, done) = (&next, &done);
    std::thread::scope(|scope| {
        let runs: Vec<_> = (0..copies)
            .map(|worker| scope.spawn(move || work(place, jobs, worker, each, next, done)))
            .collect();
        for run in runs {
            run.join().map_err(|_| Error::internal("a worker of mutate panicked"))??;
        }
        Ok::<(), Error>(())
    })?;
    let done = std::mem::take(&mut *done.lock().map_err(|_| Error::internal("a worker of mutate panicked"))?);
    Ok(done.into_iter().map(|run| run.expect("every job was taken by a worker")).collect())
}

/// One worker: a copy of its own, and the next job until there are none.
fn work(
    place: &Place,
    jobs: &[CodeJob],
    worker: usize,
    adapter_workers: usize,
    next: &AtomicUsize,
    done: &Mutex<Vec<Option<JobRun>>>,
) -> Result<()> {
    let mut scratch =
        Scratch::beside(place.project, place.own, worker).map_err(|e| copy_failed(place.project, &e))?;
    loop {
        let k = next.fetch_add(1, Ordering::SeqCst);
        let Some(job) = jobs.get(k) else { return Ok(()) };
        let run = run_job(place, &mut scratch, job, adapter_workers)?;
        done.lock().map_err(|_| Error::internal("a worker of mutate panicked"))?[k] = Some(run);
    }
}

/// Make the change, run the job's proofs, and put the file back.
fn run_job(place: &Place, scratch: &mut Scratch, job: &CodeJob, adapter_workers: usize) -> Result<JobRun> {
    let original = std::fs::read_to_string(place.project.join(&job.file))
        .map_err(|e| Error::user(format!("cannot read {} to change it: {e}", job.file)))?;
    let written = |scratch: &mut Scratch, text: &str| {
        scratch
            .write(&job.file, text)
            .map_err(|e| Error::user(format!("cannot write {} in a copy of the project: {e}", job.file)))
    };
    written(scratch, &job.changed)?;
    let run = run_proofs(place, scratch.root(), &job.proofs, adapter_workers);
    written(scratch, &original)?;
    run
}

/// The outcomes of `proofs` in the project at `root`, with an adapter of its own; a change
/// that keeps the project from loading or opening fails every proof.
fn run_proofs(place: &Place, root: &Path, proofs: &[String], adapter_workers: usize) -> Result<JobRun> {
    let files = DiskFiles::new(root.to_path_buf());
    let mut state = match load_state_beside(files, place.now, place.launches) {
        Ok(state) => state,
        Err(Error::User(message)) => return Ok(JobRun::Broken(message)),
        Err(internal) => return Err(internal),
    };
    state.config.workers = adapter_workers as u64;
    let chosen: Vec<_> = state.attestations.iter().filter(|a| proofs.contains(&a.id)).collect();
    let results = results_of(&state, place.network, &chosen);
    let finished = state.adapter.finish_session();
    let results = match results {
        Ok(results) => results,
        Err(Error::User(message)) => return Ok(JobRun::Broken(message)),
        Err(internal) => return Err(internal),
    };
    finished?;
    let ran = chosen.iter().map(|a| a.id.clone()).zip(results).collect();
    Ok(JobRun::Ran(ran))
}

fn copy_failed(project: &Path, why: &std::io::Error) -> Error {
    Error::user(format!("cannot make a copy of the project beside {}: {why}", project.display()))
}
