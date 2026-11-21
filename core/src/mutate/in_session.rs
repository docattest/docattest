//! The mutants that need no change to the project on disk, so that all of them run in the
//! project's own session beside the proofs as they are: a fact that reads a file, made to
//! read what a [`ReadMutant`] changed instead, and a row of a table with one cell changed.
//! Each is an attestation of its own that stands for the proof it was made from.

use super::proofs::{Proof, ProofKey};
use crate::altered::altered_cell;
use crate::api::attestation::{Attestation, Body, Cases};
use crate::fact_files::calls::{named_in, with_path_suffix};
use crate::fact_files::mutants::ReadMutant;

/// An attestation changed from a proof's, run in the project's session.
#[derive(Debug, Clone, PartialEq)]
pub struct InSession {
    pub proof: ProofKey,
    pub description: String,
    /// The column a changed cell is in.
    pub column: Option<String>,
    pub attestation: Attestation,
}

/// For each fact among `proofs` and each mutant of a call it makes, the fact reading the
/// changed value.
pub fn read_trials(proofs: &[Proof], mutants: &[ReadMutant]) -> Vec<InSession> {
    let mut out = Vec::new();
    for proof in proofs {
        let Body::Fact(fact) = &proof.attestation.body else { continue };
        let calls: Vec<_> = named_in(&fact.expression).collect();
        for mutant in mutants.iter().filter(|m| calls.contains(&m.call)) {
            let mut attestation = proof.attestation.clone();
            attestation.id = format!("{}{}", attestation.id, mutant.suffix);
            if let Body::Fact(changed) = &mut attestation.body {
                changed.expression = with_path_suffix(&fact.expression, &mutant.call, &mutant.suffix);
            }
            out.push(InSession {
                proof: proof.key(),
                description: mutant.description.clone(),
                column: None,
                attestation,
            });
        }
    }
    out
}

/// For each row among `proofs` and each of its cells but the first, which names the row,
/// the row alone with that cell changed. A runner that passes the row whatever the cell
/// says does not read it.
pub fn cell_trials(proofs: &[Proof]) -> Vec<InSession> {
    let mut out = Vec::new();
    for proof in proofs {
        let (Some(row), Body::Cases(cases)) = (proof.row, &proof.attestation.body) else { continue };
        for (k, (column, cell)) in row.values.iter().enumerate().skip(1) {
            let changed = altered_cell(cell);
            let mut changed_row = row.clone();
            changed_row.values[k].1.clone_from(&changed);
            let mut attestation = proof.attestation.clone();
            attestation.id = format!("{}#row{}cell{k}", attestation.id, row.line);
            attestation.body = Body::Cases(Cases { rows: vec![changed_row], ..cases.clone() });
            let description = format!("cell {column}: {cell} → {changed}");
            out.push(InSession {
                proof: proof.key(),
                description,
                column: Some(column.clone()),
                attestation,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::fact_files::load_fact_files;
    use crate::files::memory::MemoryFiles;
    use crate::files::project_files::ProjectFiles;
    use crate::mutate::proofs::proofs_on;
    use crate::project::plan::plan_project;

    const PAGE: &str = "Version `2.4.0`. <!-- fact: file(\"VERSION\").trim() -->\n\nN is `3`. <!-- fact: N -->\n\n<!-- cases: double -->\n| case | n | result |\n|---|---|---|\n| one | `1` | 2 |\n";

    fn planned() -> (MemoryFiles, Vec<Attestation>) {
        let config = r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#;
        let files = MemoryFiles::new(
            [("docattest.config.json", config), ("docs/a.md", PAGE), ("VERSION", "2.4.0\n")]
                .map(|(path, text)| (path.to_string(), text.to_string())),
        );
        let config = crate::config::parse_config(&files.read("docattest.config.json").unwrap()).unwrap();
        let plan = plan_project(&files, &config, &crate::adapter::fake::typescript()).unwrap();
        (files, plan.attestations)
    }

    #[test]
    fn a_fact_that_reads_a_file_is_made_to_read_each_change_of_it_and_no_other_fact_is() {
        let (files, attestations) = planned();
        let mut fact_files = load_fact_files(&files, &attestations);
        let added = fact_files.add_mutants(8);
        let proofs = proofs_on(&attestations, &HashSet::from(["docs/a.md".to_string()]));
        let trials = read_trials(&proofs, &added.mutants);
        assert_eq!(trials.len(), 1);
        let Body::Fact(fact) = &trials[0].attestation.body else { panic!("{trials:?}") };
        assert_eq!(fact.expression, "file(\"VERSION#mutant1\").trim()");
        assert_eq!(trials[0].description, "VERSION:1: 2.4.0 → 2.4.1");
        assert_eq!(trials[0].proof, proofs[0].key());
        assert_ne!(trials[0].attestation.id, proofs[0].attestation.id);
        assert_eq!(fact_files.session_params()["texts"]["VERSION#mutant1"], "2.4.1\n");
    }

    #[test]
    fn a_row_is_run_alone_with_each_cell_but_its_name_changed() {
        let (_, attestations) = planned();
        let proofs = proofs_on(&attestations, &HashSet::from(["docs/a.md".to_string()]));
        let trials = cell_trials(&proofs);
        let described: Vec<(&str, Option<&str>)> =
            trials.iter().map(|t| (t.description.as_str(), t.column.as_deref())).collect();
        assert_eq!(described, [("cell n: `1` → `2`", Some("n")), ("cell result: 2 → 3", Some("result"))]);
        let Body::Cases(cases) = &trials[1].attestation.body else { panic!("{trials:?}") };
        assert_eq!(cases.rows.len(), 1);
        assert_eq!(cases.rows[0].values[0], ("case".to_string(), "one".to_string()));
        assert_eq!(cases.rows[0].values[2], ("result".to_string(), "3".to_string()));
        assert_eq!(trials[0].proof, ProofKey { attestation: proofs[2].attestation.id.clone(), row: Some(8) });
    }
}
