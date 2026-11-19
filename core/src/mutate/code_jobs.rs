//! From the edits the adapter answered to the jobs [`super::workers`] runs: each edit made to
//! its file's text, with the proofs that read the symbol it changes; and, for a proof whose
//! symbols gave no edit, why.

use std::collections::{HashMap, HashSet};

use super::code::{Edit, NoEdits, SymbolEdits};
use super::proofs::ProofKey;
use super::workers::CodeJob;
use crate::api::errors::Result;
use crate::files::project_files::ProjectFiles;
use crate::project::check::SymbolRef;

/// The jobs to run, and why the symbols that gave none gave none.
#[derive(Debug, Default)]
pub struct CodePlan {
    pub jobs: Vec<CodeJob>,
    /// For each job, in order: what it changes, for the report, and the proofs it is for.
    pub described: Vec<(String, Vec<ProofKey>)>,
    /// For each proof, the symbols the adapter said it has no edit for, and why.
    pub said: HashMap<ProofKey, Vec<(NoEdits, String)>>,
    /// For each proof, why else what it reads was not changed.
    pub whys: HashMap<ProofKey, Vec<String>>,
}

/// The symbols the passing proofs read, each once, in the order they are first read.
pub fn symbols_read(reading: &[(ProofKey, Vec<SymbolRef>)]) -> Vec<SymbolRef> {
    let mut out: Vec<SymbolRef> = Vec::new();
    for symbol in reading.iter().flat_map(|(_, symbols)| symbols) {
        if !out.contains(symbol) {
            out.push(symbol.clone());
        }
    }
    out
}

/// The plan for `answers`, one for each of `symbols`. `reading` says which symbols each
/// proof reads; `own` which files are the project's own, which alone a copy may change.
pub fn plan(
    files: &dyn ProjectFiles,
    own: &HashSet<String>,
    reading: &[(ProofKey, Vec<SymbolRef>)],
    symbols: &[SymbolRef],
    answers: Vec<SymbolEdits>,
) -> Result<CodePlan> {
    let mut plan = CodePlan::default();
    for (symbol, answer) in symbols.iter().zip(answers) {
        let readers: Vec<&ProofKey> =
            reading.iter().filter(|(_, read)| read.contains(symbol)).map(|(key, _)| key).collect();
        match answer {
            SymbolEdits::None(why) => {
                for key in &readers {
                    plan.said.entry((*key).clone()).or_default().push((why, symbol.name.clone()));
                }
            }
            SymbolEdits::Edits(edits) => {
                for edit in edits {
                    if own.contains(&edit.path) {
                        plan.add_job(files, &symbol.name, &edit, &readers)?;
                    } else {
                        let why = format!(
                            "{} is declared in {}, which is not one of the project's own files",
                            symbol.name, edit.path
                        );
                        plan.explain(&readers, why);
                    }
                }
            }
        }
    }
    Ok(plan)
}

/// Why no edit of a symbol is run, for each proof that reads it.
pub fn explain_all(reading: &[(ProofKey, Vec<SymbolRef>)], why: &str) -> HashMap<ProofKey, Vec<String>> {
    reading
        .iter()
        .filter(|(_, symbols)| !symbols.is_empty())
        .map(|(key, _)| (key.clone(), vec![why.to_string()]))
        .collect()
}

impl CodePlan {
    /// Why nothing the proof `key` reads of the code was changed: the symbols the adapter
    /// gave no edit for, by reason, then anything else; None when nothing says why.
    pub fn why_not(&self, key: &ProofKey) -> Option<String> {
        let said = self.said.get(key).map_or(&[][..], Vec::as_slice);
        let mut phrases: Vec<String> = NoEdits::ALL
            .into_iter()
            .filter_map(|reason| {
                let names: Vec<String> =
                    said.iter().filter(|(why, _)| *why == reason).map(|(_, name)| name.clone()).collect();
                (!names.is_empty()).then(|| reason.phrase(&names))
            })
            .collect();
        phrases.extend(self.whys.get(key).into_iter().flatten().cloned());
        (!phrases.is_empty()).then(|| phrases.join("; "))
    }

    fn explain(&mut self, readers: &[&ProofKey], why: String) {
        for key in readers {
            let whys = self.whys.entry((*key).clone()).or_default();
            if !whys.contains(&why) {
                whys.push(why.clone());
            }
        }
    }

    fn add_job(
        &mut self,
        files: &dyn ProjectFiles,
        name: &str,
        edit: &Edit,
        readers: &[&ProofKey],
    ) -> Result<()> {
        let text = files.read(&edit.path).map_err(crate::api::errors::Error::user)?;
        let changed = edit.applied_to(&text)?;
        let mut proofs: Vec<String> = Vec::new();
        for key in readers {
            if !proofs.contains(&key.attestation) {
                proofs.push(key.attestation.clone());
            }
        }
        let description = format!("{}:{} {name}: {}", edit.path, edit.line_in(&text), edit.description);
        self.jobs.push(CodeJob { file: edit.path.clone(), changed, proofs });
        self.described.push((description, readers.iter().map(|key| (*key).clone()).collect()));
        Ok(())
    }

    /// The plan without the jobs' runs for `failing`, the proofs that do not pass in a copy
    /// of the project as it is: a change there would tell nothing about them.
    pub fn without(mut self, failing: &HashSet<ProofKey>, why: &str) -> CodePlan {
        for key in failing {
            self.said.remove(key);
            self.whys.insert(key.clone(), vec![why.to_string()]);
        }
        let mut jobs = Vec::new();
        let mut described = Vec::new();
        for (mut job, (description, keys)) in self.jobs.into_iter().zip(self.described) {
            let keys: Vec<ProofKey> = keys.into_iter().filter(|key| !failing.contains(key)).collect();
            job.proofs.retain(|id| keys.iter().any(|key| key.attestation == *id));
            if !keys.is_empty() {
                jobs.push(job);
                described.push((description, keys));
            }
        }
        CodePlan { jobs, described, said: self.said, whys: self.whys }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::memory::MemoryFiles;

    fn edit(path: &str) -> Edit {
        Edit { path: path.into(), start: 10, end: 11, replacement: "4".into(), description: "3 → 4".into() }
    }

    #[test]
    fn only_a_file_of_the_projects_own_is_changed_and_a_symbol_without_edits_says_why() {
        let files = MemoryFiles::new(
            [("src/a.ts", "const N = 3;"), ("dist/a.js", "const M = 3;")]
                .map(|(p, t)| (p.to_string(), t.to_string())),
        );
        let own = HashSet::from(["src/a.ts".to_string()]);
        let symbol = |name: &str| SymbolRef { name: name.into(), surface: None };
        let key = ProofKey { attestation: "f".into(), row: None };
        let reading = [(key.clone(), vec![symbol("N"), symbol("M"), symbol("F")])];
        let symbols = [symbol("N"), symbol("M"), symbol("F")];
        let answers = vec![
            SymbolEdits::Edits(vec![edit("src/a.ts")]),
            SymbolEdits::Edits(vec![edit("dist/a.js")]),
            SymbolEdits::None(NoEdits::NoLiteral),
        ];
        let plan = plan(&files, &own, &reading, &symbols, answers).unwrap();
        assert_eq!(
            plan.jobs,
            [CodeJob { file: "src/a.ts".into(), changed: "const N = 4;".into(), proofs: vec!["f".into()] }]
        );
        assert_eq!(plan.described, [("src/a.ts:1 N: 3 → 4".to_string(), vec![key.clone()])]);
        assert_eq!(
            plan.why_not(&key).unwrap(),
            "F is declared with no literal; M is declared in dist/a.js, which is not one of the project's own files"
        );
        let without = plan.without(&HashSet::from([key.clone()]), "it does not pass in a copy");
        assert!(without.jobs.is_empty());
        assert_eq!(without.why_not(&key).unwrap(), "it does not pass in a copy");
    }
}
