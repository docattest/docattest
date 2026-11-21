//! Mutating `console check` blocks: run each block once as baseline, then mutate its
//! captured output lines in memory against the block's expected transcript. A vacuous block
//! (such as one whose expected output is only `...`) survives; a block with real expected
//! lines is killed, and any output lines hidden by `...` are counted informatively.

use std::collections::HashMap;

use super::MUTANTS_PER_READ;
use super::effect::Effect;
use super::proofs::{Proof, ProofKey};
use super::verdict::Trial;
use crate::altered::altered_text;
use crate::api::attestation::Body;
use crate::console::block::{ConsolePlace, run_console_captured};
use crate::console::matching::first_mismatch;
use crate::console::transcript::Step;
use crate::project::state::State;
use crate::report::Status;

/// Outcome of running and mutating the `console check` proofs on the selected pages.
#[derive(Default)]
pub(super) struct ConsoleRun {
    pub baselines: HashMap<ProofKey, Status>,
    pub trials: HashMap<ProofKey, Vec<Trial>>,
    pub hidden_lines: HashMap<ProofKey, usize>,
    pub mutants: usize,
}

/// Run each `console check` proof once and mutate each passing block's captured output in
/// memory against `first_mismatch`.
pub(super) fn run_console_mutants(state: &State, proofs: &[Proof]) -> ConsoleRun {
    let mut out = ConsoleRun::default();
    let place = ConsolePlace {
        root: state.files.root(),
        timeout_ms: state.config.timeout_ms,
        adapters: state.launches.variables(),
    };
    for proof in proofs {
        let Body::Console(console) = &proof.attestation.body else { continue };
        let key = proof.key();
        let (outcome, captured) = run_console_captured(proof.attestation, console, &place);
        out.baselines.insert(key.clone(), outcome.status);
        if outcome.status != Status::Pass {
            continue;
        }
        let (trials, hidden) = mutate_steps(&console.steps, &captured, MUTANTS_PER_READ);
        out.mutants += trials.len();
        if hidden > 0 {
            out.hidden_lines.insert(key.clone(), hidden);
        }
        out.trials.insert(key, trials);
    }
    out
}

/// Test up to `limit` in-memory mutations of the captured output lines of `steps` against
/// each step's expected output; returns the trials and how many output lines `...` hid.
fn mutate_steps(steps: &[Step], captured: &[Vec<String>], limit: usize) -> (Vec<Trial>, usize) {
    let mut trials = Vec::new();
    let mut hidden = 0;
    for (step, actual) in steps.iter().zip(captured) {
        if trials.len() >= limit {
            break;
        }
        let non_blank: Vec<usize> =
            actual.iter().enumerate().filter(|(_, l)| !l.trim().is_empty()).map(|(k, _)| k).collect();
        if non_blank.is_empty() {
            let mutated = vec!["x".to_string()];
            let killed = first_mismatch(&step.output, &mutated).is_some();
            trials.push(Trial {
                description: format!("`{}`: empty output → \"x\"", step.command),
                effect: if killed { Effect::Killed } else { Effect::Passed },
                column: None,
            });
            continue;
        }
        for line_idx in non_blank {
            if trials.len() >= limit {
                break;
            }
            let was = &actual[line_idx];
            let changed = altered_text(was);
            let mut mutated = actual.clone();
            mutated[line_idx].clone_from(&changed);
            let killed = first_mismatch(&step.output, &mutated).is_some();
            if !killed {
                hidden += 1;
            }
            trials.push(Trial {
                description: format!("`{}` output line {}: {was} → {changed}", step.command, line_idx + 1),
                effect: if killed { Effect::Killed } else { Effect::Passed },
                column: None,
            });
        }
    }
    (trials, hidden)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(expected: &[&str]) -> Step {
        Step {
            line: 1,
            command: "echo hi".into(),
            output: expected.iter().map(|s| s.to_string()).collect(),
            exit: 0,
        }
    }

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_output_is_killed_and_hidden_lines_are_counted() {
        let (trials, hidden) =
            mutate_steps(&[step(&["a", "...", "c"])], &[lines(&["a", "b1", "b2", "c"])], 64);
        assert_eq!(trials.len(), 4);
        assert_eq!(trials[0].effect, Effect::Killed);
        assert_eq!(trials[1].effect, Effect::Passed);
        assert_eq!(trials[2].effect, Effect::Passed);
        assert_eq!(trials[3].effect, Effect::Killed);
        assert_eq!(hidden, 2);
    }

    #[test]
    fn a_block_with_only_ellipsis_survives_every_output_mutation() {
        let (trials, hidden) = mutate_steps(&[step(&["..."])], &[lines(&["anything"])], 64);
        assert_eq!(trials.len(), 1);
        assert_eq!(trials[0].effect, Effect::Passed);
        assert_eq!(hidden, 1);
    }

    #[test]
    fn empty_expected_output_is_killed_by_non_empty_output() {
        let (trials, hidden) = mutate_steps(&[step(&[])], &[lines(&[])], 64);
        assert_eq!(trials.len(), 1);
        assert_eq!(trials[0].effect, Effect::Killed);
        assert_eq!(hidden, 0);
    }
}
