//! Facts, and what a surface offers.
//!
//! A fact expression is a Rust expression. It is compiled in a program that imports
//! everything its surface's module exports (`use ::shop::*`), so the exports are in scope
//! by their own names and nothing else of the crate is; the setup module's `pub` items
//! are in scope the same way; and `std` and the crates of the workspace are there by
//! their names, as everywhere. The program names every crate from the root (`::shop`):
//! a crate may declare a module of its own name, which the glob import brings in.

use super::Runner;
use super::generated::CRATE;
use super::programs::{Built, Line, Render, Rendered};
use crate::analysis::surfaces::Surface;
use crate::language::text::identifiers;
use crate::protocol::{Error, Evaluation, Fact, Observable, Observation};
use serde_json::{Value, json};
use std::collections::HashMap;

/// The lines of a facts program that come before its first fact.
const PREAMBLE: usize = 7;
/// The surface a fact without `@` reads (PROTOCOL.md, `observables`).
const DEFAULT_SURFACE: &str = "default";
/// Hexadecimal digits after `0x` that make an address: fewer is a number someone wrote.
const MIN_ADDRESS_DIGITS: usize = 6;

impl Runner {
    /// Answers `evaluate`: an Evaluation for every fact, by id.
    pub(crate) fn evaluate(&self, facts: &[Fact]) -> HashMap<String, Evaluation> {
        let mut out = HashMap::new();
        let mut groups: Vec<(&Surface, Vec<&Fact>)> = Vec::new();
        for fact in facts {
            match self.refusal(fact) {
                Some(refusal) => drop(out.insert(fact.id.clone(), Evaluation::failed(refusal))),
                None => match groups.iter_mut().find(|(surface, _)| surface.name == fact.surface) {
                    Some((_, group)) => group.push(fact),
                    None => groups
                        .extend(self.analysis.surface(&fact.surface).map(|surface| (surface, vec![fact]))),
                },
            }
        }
        // Each surface's facts are a program of their own: built together, in one run of
        // cargo, and run side by side.
        let expressions: Vec<Vec<String>> = groups
            .iter()
            .map(|(_, group)| group.iter().map(|fact| fact.expression.clone()).collect())
            .collect();
        let renders: Vec<_> = groups
            .iter()
            .zip(&expressions)
            .map(|((surface, _), expressions)| {
                move |live: &[bool]| self.facts_main(surface, expressions, live, "facts")
            })
            .collect();
        let specs: Vec<(usize, Render)> = renders
            .iter()
            .zip(&expressions)
            .map(|(render, expressions)| (expressions.len(), render as Render))
            .collect();
        let programs = self.build_programs(&specs);
        let answered: Vec<HashMap<String, Evaluation>> = std::thread::scope(|scope| {
            let running: Vec<_> = groups
                .iter()
                .zip(programs)
                .map(|((surface, group), program)| {
                    scope.spawn(move || self.read_group(surface, group, program))
                })
                .collect();
            running.into_iter().map(|thread| thread.join().unwrap_or_default()).collect()
        });
        out.extend(answered.into_iter().flatten());
        out
    }

    /// Why a fact cannot be compiled at all, or `None`.
    fn refusal(&self, fact: &Fact) -> Option<String> {
        if let Some(missing) = self.analysis.not_found(&fact.surface) {
            return Some(missing);
        }
        if let Err(err) = syn::parse_str::<syn::Expr>(&fact.expression) {
            return Some(format!("cannot parse fact expression `{}`: {err}", fact.expression));
        }
        let (surface, helpers) = (self.analysis.surface(&fact.surface)?, self.analysis.setup_names());
        let both = identifiers(&fact.expression)
            .into_iter()
            .find(|name| self.analysis.exported(surface, name) && helpers.iter().any(|h| h == name))?;
        Some(format!(
            "fact `{}`: \"{both}\" is both an export of surface \"{}\" and of the setup module; rename the one in the setup module",
            fact.expression, fact.surface
        ))
    }

    /// Reads the facts of one surface from the program built for them.
    fn read_group(&self, surface: &Surface, facts: &[&Fact], program: Built) -> HashMap<String, Evaluation> {
        let mut out = HashMap::new();
        let mut kept: Vec<&Fact> = Vec::new();
        for (k, fact) in facts.iter().enumerate() {
            let label = format!("fact `{}`", fact.expression);
            match (program.dropped.get(&k), &program.binary) {
                (Some(complaint), _) => drop(out.insert(
                    fact.id.clone(),
                    Evaluation::failed(format!("{label}: {}", not_exported(complaint, surface))),
                )),
                (None, None) => {
                    let why = program.failure.clone().unwrap_or_default();
                    out.insert(
                        fact.id.clone(),
                        Evaluation::failed(format!("surface \"{}\" failed to load: {why}", surface.name)),
                    );
                }
                (None, Some(_)) => kept.push(fact),
            }
        }
        let Some(binary) = program.binary.filter(|_| !kept.is_empty()) else { return out };
        let answers = self.serve(&binary, None, kept.len());
        for (k, fact) in kept.iter().enumerate() {
            let evaluation = match &answers {
                Ok(answers) => self.evaluation(fact, &answers[k]),
                Err(failed) => Evaluation::failed(failed.clone()),
            };
            out.insert(fact.id.clone(), evaluation);
        }
        out
    }

    fn evaluation(&self, fact: &Fact, answer: &Line) -> Evaluation {
        let label = format!("fact `{}`", fact.expression);
        if answer.timeout {
            return Evaluation::failed(format!("{label} {}", self.timed_out()));
        }
        if let Some(panic) = &answer.panic {
            return Evaluation::failed(format!("{label} panicked: {panic}"));
        }
        if let Some(lost) = &answer.lost {
            return Evaluation::failed(format!("{label}: {lost}"));
        }
        match serde_json::from_value::<Observation>(answer.value.clone()) {
            Ok(value) => Evaluation::of(value),
            Err(_) => Evaluation::failed(format!("{label}: the program returned no value")),
        }
    }

    /// Writes the main of a program that observes expressions, each on a line of its
    /// own, for those still in it. `serves` is the harness function they are given to:
    /// `facts`, or `observables`, which takes each with its name.
    fn facts_main(&self, surface: &Surface, expressions: &[String], live: &[bool], serves: &str) -> Rendered {
        let mut source = format!(
            "#![allow(unused, ambiguous_glob_imports)]\nuse ::{CRATE}::harness::serve::{{manifest, snapshot}};\nuse ::{CRATE}::observe;\nuse ::{}::*;\n{}\nfn main() {{\n    ::{CRATE}::harness::serve::{serves}(vec![\n",
            surface.import,
            self.setup_module()
                .map_or_else(|| "// no setup module".to_string(), |module| format!("{module} use setup::*;")),
        );
        let mut lines = HashMap::new();
        for (k, expression) in expressions.iter().enumerate().filter(|(k, _)| live[*k]) {
            lines.insert(PREAMBLE + lines.len() + 1, k);
            let read = format!(
                "Box::new(|| observe!({}))",
                expression.split_whitespace().collect::<Vec<_>>().join(" ")
            );
            let item = if serves == "facts" { read } else { format!("({expression:?}, {read})") };
            source.push_str(&format!("        {item},\n"));
        }
        source.push_str("    ]);\n}\n");
        Rendered { source, lines }
    }

    /// Answers `observables`: the values a surface offers, which are those of its
    /// exports that can be shown, each with the fact expression that reads it. A surface
    /// that cannot be read is the project's mistake.
    pub(crate) fn observables(&self, name: &str) -> Result<Value, Error> {
        let user = |why: String| Error::User(format!("docattest: {why}"));
        if let Some(missing) = self.analysis.not_found(name) {
            return Err(user(missing));
        }
        let Some(surface) = self.analysis.surface(name) else { return Ok(json!([])) };
        let roots = self.roots(surface);
        if roots.is_empty() {
            return Ok(json!([]));
        }
        // An export that is not a value (a type, a function) does not compile as one,
        // and is taken out of the program like a fact that does not compile.
        let program =
            self.build_program(roots.len(), &|live| self.facts_main(surface, &roots, live, "observables"));
        let Some(binary) = program.binary else {
            return program
                .failure
                .map_or(Ok(json!([])), |why| Err(user(format!("surface \"{name}\" failed to load: {why}"))));
        };
        let answers = self
            .serve(&binary, None, 1)
            .map_err(|why| user(format!("surface \"{name}\" could not be read: {why}")))?;
        let answer = &answers[0];
        if let Some(why) =
            answer.panic.clone().or(answer.lost.clone()).or(answer.timeout.then(|| self.timed_out()))
        {
            return Err(user(format!("surface \"{name}\" could not be read: {why}")));
        }
        let found: Vec<Observable> = answer
            .value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| observable(name, item))
            .filter(|o| !shows_an_address(&o.value))
            .collect();
        Ok(json!(found))
    }

    /// What `observables` reads: a surface's exports and, on the default surface, a call of
    /// each setup helper whose name the surface does not export (a fact could not use it).
    fn roots(&self, surface: &Surface) -> Vec<String> {
        let mut roots = self.analysis.exports_of(&surface.name).unwrap_or_default();
        if surface.name == DEFAULT_SURFACE {
            let helpers = self.analysis.setup_helpers().into_iter();
            roots.extend(helpers.filter(|h| !self.analysis.exported(surface, h)).map(|h| format!("{h}()")));
        }
        roots
    }
}

/// Whether a value that is not text shows an address (`0x16b0f2f48`), as a pointer or a
/// function pointer does: it is shown differently in each run, so it is not offered.
fn shows_an_address(value: &Observation) -> bool {
    if value.kind.as_deref() == Some("text") {
        return false;
    }
    let text = value.text.as_bytes();
    text.windows(2).enumerate().any(|(k, pair)| {
        let starts = k == 0 || !(text[k - 1].is_ascii_alphanumeric() || text[k - 1] == b'_');
        let hex = text[k + 2..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
        starts && pair == b"0x" && hex >= MIN_ADDRESS_DIGITS
    })
}

/// One value a program read, as the protocol's Observable.
fn observable(surface: &str, item: &Value) -> Option<Observable> {
    Some(Observable {
        surface: surface.to_string(),
        expression: item["expression"].as_str()?.to_string(),
        path: vec![item["expression"].as_str()?.to_string()],
        value: serde_json::from_value(item["value"].clone()).ok()?,
    })
}

/// Words the compiler's "cannot find … in this scope" as what it means on a page.
fn not_exported(complaint: &str, surface: &Surface) -> String {
    let named = complaint.strip_prefix("cannot find ").and_then(|rest| rest.split('`').nth(1));
    match named.filter(|_| complaint.ends_with("in this scope")) {
        Some(name) => format!("{name} is not exported by surface \"{}\" ({complaint})", surface.name),
        None => complaint.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str, kind: Option<&str>) -> Observation {
        Observation {
            text: text.into(),
            items: None,
            description: String::new(),
            kind: kind.map(str::to_string),
        }
    }

    #[test]
    fn a_value_that_shows_an_address_is_not_offered() {
        assert!(shows_an_address(&value("[(\"a\", 0x1004f43c0)]", None)));
        assert!(shows_an_address(&value("0x16b0f2f48", None)));
        for shown in ["255", "0xFF", "x0x1004f43c0", "[\"a\"]"] {
            assert!(!shows_an_address(&value(shown, None)), "{shown}");
        }
        assert!(!shows_an_address(&value("0x1004f43c0", Some("text"))));
    }
}
