//! What has to be compiled or run: examples, facts, cases tables and what a surface
//! offers, each a program of a crate the adapter generates beside the project; and the
//! project's own tests that citations name, which `cargo test` runs in the project
//! ([`cited`]).

mod blocks;
mod cases;
mod cited;
mod facts;
mod generated;
mod observables;
mod outcomes;
mod programs;

use crate::analysis::Analysis;
use blocks::Blocks;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

/// The project's snapshots, as `open` gives them.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Snapshots {
    #[serde(default)]
    pub(crate) dir: Option<String>,
    pub(crate) data: Value,
}

/// What the core read of the project files facts name, as `open` gives it and generated
/// programs are told it, each by the arguments as the facts write them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FactFiles {
    /// The data of each manifest the facts name.
    pub(crate) manifests: Value,
    /// The text of each file the facts read with `file`; absent when none does.
    #[serde(default)]
    pub(crate) texts: Value,
    /// What each regex facts give `matched` matches, by the path, then the regex.
    #[serde(default)]
    pub(crate) matches: Value,
    /// The value of each field facts read with `manifestField`, by the path, then the keys.
    #[serde(default)]
    pub(crate) manifest_fields: Value,
}

/// How a session's programs are run.
pub(crate) struct Limits {
    pub(crate) timeout_ms: f64,
    /// How many programs of attested code may run at once.
    pub(crate) workers: usize,
    /// Whether examples tagged `network` run.
    pub(crate) network: bool,
}

/// What builds and runs a session's programs.
pub(crate) struct Runner {
    analysis: Arc<Analysis>,
    limits: Limits,
    snapshots: Snapshots,
    fact_files: FactFiles,
    slots: Slots,
    blocks: Blocks,
    /// The program that runs the rows of any table: the same for every table.
    cases: OnceLock<programs::Built>,
    /// One build of the generated crate at a time: its files are shared.
    building: Mutex<()>,
}

impl Runner {
    pub(crate) fn new(
        analysis: Arc<Analysis>,
        limits: Limits,
        snapshots: Snapshots,
        fact_files: FactFiles,
    ) -> Self {
        Runner {
            analysis,
            slots: Slots { free: Mutex::new(limits.workers.max(1)), freed: Condvar::new() },
            limits,
            snapshots,
            fact_files,
            blocks: Blocks::default(),
            cases: OnceLock::new(),
            building: Mutex::new(()),
        }
    }

    /// Waits for the build in progress: cargo holds the generated crate's files until it
    /// ends.
    pub(crate) fn wait_for_build(&self) {
        drop(self.one_build());
    }

    fn one_build(&self) -> MutexGuard<'_, ()> {
        self.building.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs_f64(self.limits.timeout_ms / 1000.0)
    }

    /// The time limit as a message writes it.
    fn timed_out(&self) -> String {
        format!("timed out after {} ms", self.limits.timeout_ms)
    }

    /// Runs a program of attested code, when one of the session's slots is free.
    fn running<T>(&self, program: impl FnOnce() -> T) -> T {
        let _slot = self.slots.take();
        program()
    }
}

/// How many programs of attested code may run at once.
struct Slots {
    free: Mutex<usize>,
    freed: Condvar,
}

impl Slots {
    /// Waits for a free slot and takes it; it is given back when the slot is dropped,
    /// even by a program that panics.
    fn take(&self) -> Slot<'_> {
        let mut free = self.free.lock().unwrap_or_else(|p| p.into_inner());
        while *free == 0 {
            free = self.freed.wait(free).unwrap_or_else(|p| p.into_inner());
        }
        *free -= 1;
        Slot { slots: self }
    }
}

/// A slot taken, until it is dropped.
struct Slot<'a> {
    slots: &'a Slots,
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        *self.slots.free.lock().unwrap_or_else(|p| p.into_inner()) += 1;
        self.slots.freed.notify_one();
    }
}
