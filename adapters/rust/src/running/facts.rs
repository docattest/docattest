//! Facts, and what a surface offers.
//!
//! A fact expression is a Rust expression. It is compiled in a program that imports
//! everything its surface's module exports (`use ::shop::*`), so the exports are in scope
//! by their own names and nothing else of the crate is, but for the module's `pub` docs
//! helpers, which the glob brings in too; the setup module's `pub` items are in scope the
//! same way; and `std` and the crates of the workspace are there by their names, as
//! everywhere. The program names every crate from the root (`::shop`): a crate may
//! declare a module of its own name, which the glob import brings in.
//!
//! A constant the surface's module does not export, and any constant of a binary crate,
//! which nothing imports, is read from source instead: its declaration is written into
//! the fact's own line of the program, in a module of the fact's closure.
//!
//! A fact that names nothing of the project's crates (no export of its surface, no name
//! of the setup module, which is compiled with them, and no crate), such as one that
//! reads only constants from source, is compiled in a program of the harness alone: it is
//! read whether the project's code compiles or not.

use super::Runner;
use super::generated::{Dependencies, HARNESS_CRATE};
use super::programs::{Built, Line, Program, Rendered};
use crate::analysis::surfaces::Surface;
use crate::language::text::identifiers;
use crate::protocol::{Evaluation, Fact, Observation};
use std::collections::HashMap;

/// The lines of a facts program that come before its first fact.
const PREAMBLE: usize = 7;
/// The surface a fact without `@` reads (PROTOCOL.md, `observables`).
pub(super) const DEFAULT_SURFACE: &str = "default";
/// The module of a fact's closure that declares the constants it reads from source.
const SOURCE: &str = "docattest_from_source";

/// What a program of expressions reads: a surface (if any), through what the program is built with.
#[derive(Clone, Copy)]
pub(super) struct Scope<'a> {
    pub(super) surface: Option<&'a Surface>,
    pub(super) dependencies: Dependencies,
}

/// The facts one program reads.
struct Group<'a> {
    scope: Scope<'a>,
    facts: Vec<&'a Fact>,
}

impl<'a> Group<'a> {
    /// Takes the facts the compiler complained of out of a program of the harness alone,
    /// as a group to build with the project's crates; the program reads the others.
    fn take_dropped(&mut self, program: &mut Built) -> Option<Group<'a>> {
        if self.scope.dependencies != Dependencies::Harness || program.dropped.is_empty() {
            return None;
        }
        let (dropped, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.facts)
            .into_iter()
            .enumerate()
            .partition(|(k, _)| program.dropped.contains_key(k));
        self.facts = kept.into_iter().map(|(_, fact)| fact).collect();
        program.dropped.clear();
        let scope = Scope { dependencies: Dependencies::Project, ..self.scope };
        Some(Group { scope, facts: dropped.into_iter().map(|(_, fact)| fact).collect() })
    }
}

impl Runner {
    /// Answers `evaluate`: an Evaluation for every fact, by id.
    pub(crate) fn evaluate(&self, facts: &[Fact]) -> HashMap<String, Evaluation> {
        let mut out = HashMap::new();
        let mut groups: Vec<Group> = Vec::new();
        for fact in facts {
            match self.refusal(fact) {
                Some(refusal) => drop(out.insert(fact.id.clone(), Evaluation::failed(refusal))),
                None => self.place(&mut groups, fact),
            }
        }
        let mut built = self.build_groups(groups);
        // A fact that names nothing of the project's crates and does not compile without
        // them is built with them: it may use what its surface's import brings in, such
        // as a trait's methods, and what does not compile there is its error.
        let retried: Vec<Group> =
            built.iter_mut().filter_map(|(group, program)| group.take_dropped(program)).collect();
        built.extend(self.build_groups(retried));
        let answered: Vec<HashMap<String, Evaluation>> = std::thread::scope(|scope| {
            let running: Vec<_> = built
                .into_iter()
                .map(|(group, program)| {
                    scope.spawn(move || self.read_group(group.scope.surface, &group.facts, program))
                })
                .collect();
            running.into_iter().map(|thread| thread.join().unwrap_or_default()).collect()
        });
        out.extend(answered.into_iter().flatten());
        out
    }

    /// Puts a fact in the group of its surface and of what its program is built with.
    fn place<'a>(&'a self, groups: &mut Vec<Group<'a>>, fact: &'a Fact) {
        let surface = self.analysis.surface(&fact.surface);
        if surface.is_none() && !(!self.analysis.has_surfaces() && fact.surface == DEFAULT_SURFACE) {
            return;
        }
        let dependencies = match surface {
            Some(surface) => self.dependencies_of(surface, &fact.expression),
            None => self.dependencies_without_surface(&fact.expression),
        };
        let scope = Scope { surface, dependencies };
        let surface_name = surface.map(|s| s.name.as_str());
        let same = |group: &&mut Group| {
            group.scope.surface.map(|s| s.name.as_str()) == surface_name
                && group.scope.dependencies == scope.dependencies
        };
        match groups.iter_mut().find(same) {
            Some(group) => group.facts.push(fact),
            None => groups.push(Group { scope, facts: vec![fact] }),
        }
    }

    /// What a fact's program is built with when no surfaces are configured.
    fn dependencies_without_surface(&self, expression: &str) -> Dependencies {
        let (helpers, crates) = (self.analysis.setup_names(), self.crate_names());
        if identifiers(expression).into_iter().any(|name| helpers.iter().chain(&crates).any(|k| k == name)) {
            Dependencies::Project
        } else {
            Dependencies::Harness
        }
    }

    /// What a fact's program is built with: the project's crates when the fact names
    /// anything of them, else the harness alone.
    fn dependencies_of(&self, surface: &Surface, expression: &str) -> Dependencies {
        let (helpers, crates) = (self.analysis.setup_names(), self.crate_names());
        let of_project = |name: &str| {
            self.analysis.exported(surface, name) || helpers.iter().chain(&crates).any(|known| known == name)
        };
        if identifiers(expression).into_iter().any(of_project) {
            Dependencies::Project
        } else {
            Dependencies::Harness
        }
    }

    /// Builds a program for each group, together: one run of cargo for each package.
    fn build_groups<'a>(&self, groups: Vec<Group<'a>>) -> Vec<(Group<'a>, Built)> {
        let expressions: Vec<Vec<String>> = groups
            .iter()
            .map(|group| group.facts.iter().map(|fact| fact.expression.clone()).collect())
            .collect();
        let renders: Vec<_> = groups
            .iter()
            .zip(&expressions)
            .map(|(group, expressions)| {
                let scope = group.scope;
                move |live: &[bool]| self.facts_main(scope, expressions, live, "facts")
            })
            .collect();
        let programs: Vec<Program> = groups
            .iter()
            .zip(&renders)
            .map(|(group, render)| Program {
                dependencies: group.scope.dependencies,
                pieces: group.facts.len(),
                render,
            })
            .collect();
        let built = self.build_programs(&programs);
        groups.into_iter().zip(built).collect()
    }

    /// Why a fact cannot be compiled at all, or `None`.
    fn refusal(&self, fact: &Fact) -> Option<String> {
        let no_surfaces = !self.analysis.has_surfaces() && fact.surface == DEFAULT_SURFACE;
        if !no_surfaces && let Some(missing) = self.analysis.not_found(&fact.surface) {
            return Some(missing);
        }
        if let Err(err) = syn::parse_str::<syn::Expr>(&fact.expression) {
            return Some(format!("cannot parse fact expression `{}`: {err}", fact.expression));
        }
        let surface = self.analysis.surface(&fact.surface)?;
        let helpers = self.analysis.setup_names();
        let of_surface = |name: &str| {
            self.analysis.exported(surface, name) || self.analysis.read_from_source(surface, name)
        };
        let both = identifiers(&fact.expression)
            .into_iter()
            .find(|name| of_surface(name) && helpers.iter().any(|h| h == name))?;
        Some(format!(
            "fact `{}`: \"{both}\" is both an export of surface \"{}\" and of the setup module; rename the one in the setup module",
            fact.expression, fact.surface
        ))
    }

    /// Reads the facts of one surface from the program built for them.
    fn read_group(
        &self,
        surface: Option<&Surface>,
        facts: &[&Fact],
        program: Built,
    ) -> HashMap<String, Evaluation> {
        let mut out = HashMap::new();
        let mut kept: Vec<&Fact> = Vec::new();
        let surface_name = surface.map_or(DEFAULT_SURFACE, |s| s.name.as_str());
        for (k, fact) in facts.iter().enumerate() {
            let label = format!("fact `{}`", fact.expression);
            match (program.dropped.get(&k), &program.binary) {
                (Some(complaint), _) => drop(out.insert(
                    fact.id.clone(),
                    Evaluation::failed(format!("{label}: {}", self.not_read(complaint, surface))),
                )),
                (None, None) => {
                    let why = program.failure.clone().unwrap_or_default();
                    out.insert(
                        fact.id.clone(),
                        Evaluation::failed(format!("surface \"{surface_name}\" failed to load: {why}")),
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
    pub(super) fn facts_main(
        &self,
        scope: Scope,
        expressions: &[String],
        live: &[bool],
        serves: &str,
    ) -> Rendered {
        let (imported, setup) = self.project_lines(scope);
        let mut source = format!(
            "#![allow(unused, ambiguous_glob_imports)]\nuse ::{HARNESS_CRATE}::harness::serve::{{file, manifest, manifestField, matched, snapshot}};\nuse ::{HARNESS_CRATE}::observe;\n{imported}\n{setup}\nfn main() {{\n    ::{HARNESS_CRATE}::harness::serve::{serves}(vec![\n",
        );
        let mut lines = HashMap::new();
        for (k, expression) in expressions.iter().enumerate().filter(|(k, _)| live[*k]) {
            lines.insert(PREAMBLE + lines.len() + 1, k);
            let read =
                self.reader(scope.surface, &expression.split_whitespace().collect::<Vec<_>>().join(" "));
            let item = if serves == "facts" { read } else { format!("({expression:?}, {read})") };
            source.push_str(&format!("        {item},\n"));
        }
        source.push_str("    ]);\n}\n");
        Rendered { source, lines }
    }

    /// The two lines of a facts program that bring in the project's crates: the import of
    /// the surface's module, and the setup module.
    fn project_lines(&self, scope: Scope) -> (String, String) {
        if scope.dependencies == Dependencies::Harness {
            return (
                "// built with the harness alone:".to_string(),
                "// nothing of the project's crates".to_string(),
            );
        }
        let imported = match scope.surface {
            Some(surface) if surface.importable() => format!("use ::{}::*;", surface.import),
            Some(_) => "// a binary crate: nothing to import".to_string(),
            None => "// no surface configured".to_string(),
        };
        let setup = self
            .setup_module()
            .map_or_else(|| "// no setup module".to_string(), |module| format!("{module} use setup::*;"));
        (imported, setup)
    }

    /// The closure that reads an expression, on one line, so that what the compiler says
    /// of any of it is said of its fact: with the declarations of the constants it reads
    /// from source, in a module of its own.
    fn reader(&self, surface: Option<&Surface>, expression: &str) -> String {
        let Some(surface) = surface else {
            return format!("Box::new(|| observe!({expression}))");
        };
        let constants = self.analysis.source_constants(surface, expression);
        if constants.names.is_empty() {
            return format!("Box::new(|| observe!({expression}))");
        }
        format!(
            "Box::new(|| {{ mod {SOURCE} {{ {} }} use {SOURCE}::{{{}}}; observe!({expression}) }})",
            constants.declarations.join(" "),
            constants.names.join(", ")
        )
    }

    /// Words the compiler's "cannot find … in this scope" as what it means on a page: the
    /// name is not exported, or is declared but not read from source, and why.
    fn not_read(&self, complaint: &str, surface: Option<&Surface>) -> String {
        let named = complaint.strip_prefix("cannot find ").and_then(|rest| rest.split('`').nth(1));
        let Some(name) = named.filter(|_| complaint.ends_with("in this scope")) else {
            return complaint.to_string();
        };
        match surface {
            Some(surface) => self.analysis.unread(surface, name).unwrap_or_else(|| {
                format!("{name} is not exported by surface \"{}\" ({complaint})", surface.name)
            }),
            None => format!("{name} is not in scope ({complaint})"),
        }
    }
}
