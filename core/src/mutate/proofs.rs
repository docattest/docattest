//! The proofs `mutate` judges: every attestation on the chosen pages, each row of a table a
//! proof of its own, as `check` reports them.

use std::collections::HashSet;

use crate::api::attestation::{Attestation, Body, CasesRow};
use crate::report::{Outcome, Status};

/// Which proof: its attestation, and for a row of a table, the row's line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProofKey {
    pub attestation: String,
    pub row: Option<usize>,
}

/// One proof on a chosen page.
#[derive(Debug, Clone, Copy)]
pub struct Proof<'a> {
    pub attestation: &'a Attestation,
    pub row: Option<&'a CasesRow>,
}

impl Proof<'_> {
    pub fn key(&self) -> ProofKey {
        ProofKey { attestation: self.attestation.id.clone(), row: self.row.map(|row| row.line) }
    }

    /// The line `check` reports it at.
    pub fn line(&self) -> usize {
        self.row.map_or(self.attestation.line, |row| row.line)
    }
}

/// How a proof is run, which decides whether `mutate` can change what it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    /// The adapter runs or evaluates it: a fact, an example, a row of a table.
    Adapter,
    /// A citation, which docattest judges from the test file without running the test.
    Citation,
    /// A block the core runs itself (a console block, a sample page): what it reads is not
    /// known.
    Core,
}

/// The proofs on the `selected` pages, in plan order: an attestation, or each row of a
/// table that could be planned.
pub fn proofs_on<'a>(attestations: &'a [Attestation], selected: &HashSet<String>) -> Vec<Proof<'a>> {
    let mut out = Vec::new();
    for attestation in attestations.iter().filter(|a| selected.contains(&a.page)) {
        match &attestation.body {
            Body::Cases(cases) if attestation.ok() => {
                out.extend(cases.rows.iter().map(|row| Proof { attestation, row: Some(row) }));
            }
            _ => out.push(Proof { attestation, row: None }),
        }
    }
    out
}

/// How `attestation` is run.
pub fn runner(attestation: &Attestation) -> Runner {
    match &attestation.body {
        Body::Fact(_) | Body::Example(_) | Body::Cases(_) => Runner::Adapter,
        Body::Test(_) => Runner::Citation,
        Body::Console(_) | Body::Sample(_) => Runner::Core,
    }
}

/// The outcomes of the proof `key` among its attestation's: a row's own, else all of them.
pub fn outcomes_of<'o>(key: &ProofKey, outcomes: &'o [Outcome]) -> Vec<&'o Outcome> {
    outcomes.iter().filter(|o| key.row.is_none_or(|line| o.line == line)).collect()
}

/// How a proof went before anything was changed: `Pass` when every outcome of it passed,
/// else the first status that is not a pass. A proof with no outcome did not pass.
pub fn baseline(outcomes: &[&Outcome]) -> Status {
    let other = outcomes.iter().map(|o| o.status).find(|status| *status != Status::Pass);
    match (outcomes.is_empty(), other) {
        (true, _) => Status::Error,
        (false, Some(status)) => status,
        (false, None) => Status::Pass,
    }
}

/// Whether a mutant made the proof fail: some outcome of it failed or was an error.
pub fn failed(outcomes: &[&Outcome]) -> bool {
    outcomes.iter().any(|o| o.is_failure())
}
