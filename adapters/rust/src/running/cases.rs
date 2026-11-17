//! The setup module, and the cases tables its runners run.
//!
//! The setup of a Rust project is one Rust file, named in the config. It is no part of
//! the project's crates: docattest compiles it, as a module of a program of its own
//! that depends on them. It may declare
//!
//! ```text
//! pub const CASES: &[(&str, fn(&HashMap<String, String>) -> Result<(), String>)] = &[…];
//! ```
//!
//! the runners of cases tables by name, `pub const COLUMNS: &[(&str, &[&str])]`, the
//! columns each runner's table must have, and any other `pub` item, which fact
//! expressions may use by name.

use super::Runner;
use super::generated::CRATE;
use super::outcomes::Outcomes;
use super::programs::{Built, Line, Rendered};
use crate::protocol::{Cases, Kind, Status, Verdict};
use serde_json::json;
use std::collections::HashMap;

impl Runner {
    /// The declaration that makes the setup file a module of a generated program.
    pub(super) fn setup_module(&self) -> Option<String> {
        let path = self.analysis.abs(self.analysis.setup.as_ref()?);
        path.is_file().then(|| format!("#[path = {:?}] mod setup;", path.to_string_lossy()))
    }

    /// The program that runs the rows of any table of the project, built once.
    fn cases_program(&self) -> Built {
        let built = self.cases.get_or_init(|| {
            let columns = if self.analysis.setup_names().iter().any(|name| name == "COLUMNS") { "setup::COLUMNS" } else { "&[]" };
            let source = format!(
                "#![allow(unused)]\n{}\nfn main() {{\n    ::{CRATE}::harness::serve::cases(setup::CASES, {columns});\n}}\n",
                self.setup_module().unwrap_or_default()
            );
            self.build_program(0, &|_| Rendered { source: source.clone(), lines: HashMap::new() })
        });
        built.clone()
    }

    /// Runs a cases table: a result for each row at the row's line, or one error at the
    /// table's line when the table cannot be run at all.
    pub(super) fn run_cases(&self, table: &Cases) -> Vec<Verdict> {
        let at = format!("cases \"{}\" at L{}", table.runner, table.line);
        let refuse = |message: String| {
            vec![Outcomes::of(Status::Error, message).verdict(&table.page, table.line, Kind::Cases)]
        };
        let Some(setup) = &self.analysis.setup else {
            return refuse(format!(
                "{at}: no setup module is configured (\"setup\" in docattest.config.json)"
            ));
        };
        if self.setup_module().is_none() {
            return refuse(format!("{at}: setup module {setup} not found"));
        }
        if table.rows.is_empty() {
            return Vec::new();
        }
        let program = self.cases_program();
        let Some(binary) = program.binary else {
            return refuse(format!(
                "{at}: setup module {setup} failed to load: {}",
                program.failure.unwrap_or_default()
            ));
        };
        let rows: Vec<_> = table.rows.iter().map(|row| json!({ "values": row.values })).collect();
        let told = json!({ "runner": table.runner, "setup": setup, "line": table.line, "header": table.header, "rows": rows });
        match self.serve(&binary, Some(told), table.rows.len()) {
            Err(failed) => refuse(failed),
            Ok(answers) => table
                .rows
                .iter()
                .zip(&answers)
                .map(|(row, answer)| {
                    let name = format!("{}:{} {}", table.page, row.line, row.name);
                    self.row_outcome(&name, answer).verdict(&table.page, row.line, Kind::Cases)
                })
                .collect(),
        }
    }

    /// Judges one row by its program's answer: the runner's complaint, or "".
    fn row_outcome(&self, name: &str, answer: &Line) -> Outcomes {
        if answer.timeout {
            return Outcomes::fail(format!("{name}: {}", self.timed_out()));
        }
        if let Some(panic) = &answer.panic {
            return Outcomes::fail(format!("{name}: panicked: {panic}"));
        }
        if let Some(lost) = &answer.lost {
            return Outcomes::fail(format!("{name}: {lost}"));
        }
        match answer.value.as_str().filter(|why| !why.is_empty()) {
            Some(why) => Outcomes::fail(format!("{name}: {why}")),
            None => Outcomes::pass(name),
        }
    }
}
