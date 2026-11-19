//! What `mutate` says of each proof, from the mutants it ran against it: killed (some mutant
//! made it fail), survived (none did) or not mutated (nothing it reads could be changed, or
//! every change left the code unable to build), and the report as `--json` gives it. A
//! stillborn mutant ([`Effect::Stillborn`]) is counted, and kills nothing.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::effect::Effect;
use super::proofs::{Proof, ProofKey};
use crate::api::attestation::Kind;
use crate::report::order::by_page_and_line;

/// One mutant run against one proof: what it changed, and what that did to the proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trial {
    pub description: String,
    pub effect: Effect,
    /// The column of a changed cell of a table.
    pub column: Option<String>,
}

/// The verdict on one proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The first mutant that made it fail, how many that built were run, how many did not
    /// build, and the columns of its row that no change made fail.
    Killed { by: String, tried: usize, stillborn: usize, ignored: Vec<String> },
    /// Every mutant that built, none of which made it fail, and how many did not build.
    Survived { survivors: Vec<String>, stillborn: usize },
    /// Why nothing it reads was changed.
    NotMutated { why: String },
}

/// A proof, where it is, and its verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    pub page: String,
    pub line: usize,
    pub kind: Kind,
    pub claim: String,
    /// A row's first cell, which names it.
    pub row: Option<String>,
    pub verdict: Verdict,
}

/// Everything `mutate` found: a verdict on each proof, how many mutants it ran, and how many
/// of those left the code unable to build or load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutation {
    pub proofs: Vec<Judged>,
    pub mutants: usize,
    pub stillborn: usize,
}

/// Why a proof whose every mutant was stillborn is not mutated.
pub const ALL_STILLBORN: &str = "every change to what it reads left the code unable to build or load, which tells nothing about the proof";

/// The verdict on a proof that passed and had `trials` run against it.
pub fn verdict_of(trials: &[Trial]) -> Verdict {
    let built: Vec<&Trial> = trials.iter().filter(|t| t.effect != Effect::Stillborn).collect();
    let stillborn = trials.len() - built.len();
    if built.is_empty() {
        return Verdict::NotMutated { why: ALL_STILLBORN.to_string() };
    }
    let Some(killer) = built.iter().find(|t| t.effect == Effect::Killed) else {
        return Verdict::Survived {
            survivors: built.iter().map(|t| t.description.clone()).collect(),
            stillborn,
        };
    };
    let mut ignored: Vec<String> = Vec::new();
    for trial in built.iter().filter(|t| t.effect == Effect::Passed) {
        if let Some(column) = trial.column.as_ref().filter(|c| !ignored.contains(c)) {
            ignored.push(column.clone());
        }
    }
    Verdict::Killed { by: killer.description.clone(), tried: built.len(), stillborn, ignored }
}

/// Each proof judged, by page and line; `verdicts` holds a verdict for every proof.
pub fn judged(
    proofs: &[Proof],
    mut verdicts: HashMap<ProofKey, Verdict>,
    mutants: usize,
    stillborn: usize,
) -> Mutation {
    let mut judged: Vec<Judged> = proofs
        .iter()
        .map(|proof| Judged {
            page: proof.attestation.page.clone(),
            line: proof.line(),
            kind: proof.attestation.kind(),
            claim: proof.attestation.claim.clone(),
            row: proof.row.map(|row| row.name.clone()),
            verdict: verdicts.remove(&proof.key()).expect("a verdict for every proof"),
        })
        .collect();
    judged.sort_by(|a, b| by_page_and_line((&a.page, a.line), (&b.page, b.line)));
    Mutation { proofs: judged, mutants, stillborn }
}

impl Mutation {
    /// How many proofs were killed, survived and were not mutated.
    pub fn counts(&self) -> (usize, usize, usize) {
        let count = |wanted: fn(&Verdict) -> bool| self.proofs.iter().filter(|p| wanted(&p.verdict)).count();
        (
            count(|v| matches!(v, Verdict::Killed { .. })),
            count(|v| matches!(v, Verdict::Survived { .. })),
            count(|v| matches!(v, Verdict::NotMutated { .. })),
        )
    }

    /// 1 when a proof survived and survivors fail the command, else 0.
    pub fn exit_code(&self, survivors_fail: bool) -> i32 {
        let (_, survived, _) = self.counts();
        i32::from(survivors_fail && survived > 0)
    }

    pub fn to_json(&self) -> Value {
        let (killed, survived, not_mutated) = self.counts();
        json!({
            "proofs": self.proofs.iter().map(Judged::to_json).collect::<Vec<_>>(),
            "summary": {
                "proofs": self.proofs.len(), "killed": killed, "survived": survived,
                "notMutated": not_mutated, "mutants": self.mutants, "stillborn": self.stillborn,
            },
        })
    }
}

impl Judged {
    fn to_json(&self) -> Value {
        let mut out =
            json!({ "page": self.page, "line": self.line, "kind": self.kind.as_str(), "claim": self.claim });
        if let Some(row) = &self.row {
            out["row"] = json!(row);
        }
        match &self.verdict {
            Verdict::Killed { by, tried, stillborn, ignored } => {
                out["status"] = json!("killed");
                out["mutants"] = json!(tried);
                out["stillborn"] = json!(stillborn);
                out["killedBy"] = json!(by);
                if !ignored.is_empty() {
                    out["ignoredCells"] = json!(ignored);
                }
            }
            Verdict::Survived { survivors, stillborn } => {
                out["status"] = json!("survived");
                out["mutants"] = json!(survivors.len());
                out["stillborn"] = json!(stillborn);
                out["survivors"] = json!(survivors);
            }
            Verdict::NotMutated { why } => {
                out["status"] = json!("notMutated");
                out["why"] = json!(why);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(description: &str, effect: Effect, column: Option<&str>) -> Trial {
        Trial { description: description.into(), effect, column: column.map(str::to_string) }
    }

    #[test]
    fn a_proof_is_killed_by_the_first_mutant_that_fails_it_and_names_the_cells_nothing_failed() {
        let trials = [
            trial("cell note: a → ax", Effect::Passed, Some("note")),
            trial("cell result: 5 → 6", Effect::Killed, Some("result")),
            trial("cell value: 1 → 2", Effect::Killed, Some("value")),
            trial("cell note: b → bx", Effect::Passed, Some("note")),
        ];
        assert_eq!(
            verdict_of(&trials),
            Verdict::Killed {
                by: "cell result: 5 → 6".into(),
                tried: 4,
                stillborn: 0,
                ignored: vec!["note".into()]
            }
        );
        let none_killed = [trial("a: 1 → 2", Effect::Passed, None), trial("a: x → xx", Effect::Passed, None)];
        assert_eq!(
            verdict_of(&none_killed),
            Verdict::Survived { survivors: vec!["a: 1 → 2".into(), "a: x → xx".into()], stillborn: 0 }
        );
    }

    #[test]
    fn a_stillborn_mutant_kills_nothing_and_a_proof_with_only_those_is_not_mutated() {
        let stillborn = trial("a: 255 → 256", Effect::Stillborn, None);
        let survived = [stillborn.clone(), trial("a: x → xx", Effect::Passed, None)];
        assert_eq!(
            verdict_of(&survived),
            Verdict::Survived { survivors: vec!["a: x → xx".into()], stillborn: 1 }
        );
        let killed = [stillborn.clone(), trial("a: 1 → 2", Effect::Killed, None)];
        assert_eq!(
            verdict_of(&killed),
            Verdict::Killed { by: "a: 1 → 2".into(), tried: 1, stillborn: 1, ignored: vec![] }
        );
        assert_eq!(verdict_of(&[stillborn]), Verdict::NotMutated { why: ALL_STILLBORN.into() });
    }

    #[test]
    fn survivors_fail_the_command_only_when_asked() {
        let judged = |verdict: Verdict| Judged {
            page: "docs/a.md".into(),
            line: 1,
            kind: Kind::Fact,
            claim: String::new(),
            row: None,
            verdict,
        };
        let survived = Mutation {
            proofs: vec![judged(Verdict::Survived { survivors: vec!["x".into()], stillborn: 0 })],
            mutants: 1,
            stillborn: 0,
        };
        assert_eq!((survived.exit_code(false), survived.exit_code(true)), (0, 1));
        let killed = Mutation {
            proofs: vec![judged(Verdict::Killed { by: "x".into(), tried: 1, stillborn: 1, ignored: vec![] })],
            mutants: 2,
            stillborn: 1,
        };
        assert_eq!(killed.exit_code(true), 0);
        assert_eq!(killed.to_json()["summary"]["stillborn"], 1);
        assert_eq!(killed.to_json()["proofs"][0]["stillborn"], 1);
    }
}
