//! `symbols`: the surface symbols an attestation names.
//!
//! Read from tokens, so that what a macro is given counts (`assert_eq!(shop::total(…))`):
//! a path that goes through a surface's module names the export after it, a `use` makes
//! a shorter name stand for such a path, and a glob import of a surface's module, or
//! code written in that module's own file, reaches its exports by their bare names.

use super::Analysis;
use super::modules::use_leaves;
use super::surfaces::Surface;
use super::tests::{cited, declared};
use crate::language::text::Token;
use crate::language::{blocks, is_source, text};
use crate::protocol::{Attestation, SymbolRef};
use quote::ToTokens;
use std::collections::HashMap;
use syn::visit::Visit;

/// The `use` declarations of a piece of code, wherever they are written.
#[derive(Default)]
struct Uses {
    /// What each name a `use` brings in stands for.
    names: HashMap<String, Vec<String>>,
    /// The modules imported whole, with a glob.
    globs: Vec<Vec<String>>,
}

impl<'ast> Visit<'ast> for Uses {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        for (name, path) in use_leaves(&item.tree) {
            if name == "*" {
                self.globs.push(path);
            } else {
                self.names.insert(name, path);
            }
        }
    }
}

/// Collects the surface symbols code names, each once, in order.
struct Refs<'a> {
    analysis: &'a Analysis,
    found: Vec<SymbolRef>,
}

impl Refs<'_> {
    /// Records a name of a surface, when the surface exports it.
    fn add(&mut self, name: &str, surface: &Surface) {
        let found = SymbolRef { name: name.to_string(), surface: surface.name.clone() };
        if self.analysis.exported(surface, name) && !self.found.contains(&found) {
            self.found.push(found);
        }
    }

    /// Adds what a piece of code names. `own` is the file the code is written in, when
    /// it is a file of the project: a surface whose module that file is reaches its
    /// exports by their bare names, and `crate` there is that surface's crate.
    fn collect(&mut self, code: &str, parsed: Option<&syn::File>, own: Option<&std::path::Path>) {
        let mut uses = Uses::default();
        if let Some(file) = parsed {
            uses.visit_file(file);
        }
        let analysis = self.analysis;
        for written in text::paths(code) {
            let path = match uses.names.get(&written[0]) {
                Some(stands_for) => stands_for.iter().chain(&written[1..]).cloned().collect(),
                None => written.clone(),
            };
            for surface in &analysis.surfaces {
                let import: Vec<&str> = surface.import.split("::").collect();
                let bare = (own.is_some() && surface.file.as_deref() == own) || uses.imports_whole(&import);
                match through(&path, &import, own.is_some()) {
                    Some(name) => self.add(name, surface),
                    None if bare => self.add(&written[0], surface),
                    None => {}
                }
            }
        }
    }
}

impl Uses {
    /// Whether a module is imported whole, with a glob.
    fn imports_whole(&self, import: &[&str]) -> bool {
        self.globs.iter().any(|glob| glob.iter().map(String::as_str).eq(import.iter().copied()))
    }
}

/// The name a path reaches through a surface's module: the segment after the module's
/// own path, written from the crate's name or, in the crate's own code, from `crate`.
fn through<'a>(path: &'a [String], import: &[&str], in_crate: bool) -> Option<&'a str> {
    let begins = |first: &str| {
        let module = std::iter::once(first).chain(import.iter().skip(1).copied());
        path.len() > import.len() && path.iter().map(String::as_str).zip(module).all(|(a, b)| a == b)
    };
    (begins(import[0]) || (in_crate && begins("crate"))).then(|| path[import.len()].as_str())
}

impl Analysis {
    /// Answers `symbols` for one attestation.
    pub(crate) fn symbols(&self, attestation: &Attestation) -> Vec<SymbolRef> {
        let mut refs = Refs { analysis: self, found: Vec::new() };
        match attestation.kind.as_str() {
            "example" => {
                let own = is_source(&attestation.page).then(|| self.abs(&attestation.page));
                if let Some(owner) = attestation.owner.as_ref().filter(|_| own.is_some()) {
                    for surface in self.surfaces.iter().filter(|surface| surface.file == own) {
                        refs.add(owner, surface);
                    }
                }
                refs.collect(
                    &blocks::visible(&attestation.code),
                    blocks::parsed(&attestation.code).as_ref(),
                    None,
                );
            }
            "fact" => {
                if let Some(surface) = self.surface(&attestation.surface) {
                    text::identifiers(&attestation.expression)
                        .into_iter()
                        .for_each(|name| refs.add(name, surface));
                }
            }
            "test" => self.citation_symbols(&mut refs, attestation),
            "cases" => self.runner_symbols(&mut refs, &attestation.runner),
            _ => {}
        }
        refs.found
    }

    /// What the test a citation names uses.
    fn citation_symbols(&self, refs: &mut Refs, attestation: &Attestation) {
        let path = self.abs(&attestation.file);
        let Some(file) = std::fs::read_to_string(&path).ok().and_then(|text| syn::parse_file(&text).ok())
        else {
            return;
        };
        let all = declared(&file);
        if let Some(function) =
            cited(&all, &attestation.title, &attestation.describes).and_then(|test| test.function)
        {
            refs.collect(&function.to_token_stream().to_string(), Some(&file), Some(&path));
        }
    }

    /// What the runner of a cases table uses: the function the setup module's CASES
    /// gives for its name.
    fn runner_symbols(&self, refs: &mut Refs, runner: &str) {
        let Some(file) = self.setup_file() else { return };
        let functions = file.items.iter().filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function),
            _ => None,
        });
        let named: Vec<String> = file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Const(cases) if cases.ident == "CASES" => {
                    Some(cases.expr.to_token_stream().to_string())
                }
                _ => None,
            })
            .flat_map(|cases| runner_names(&cases, runner))
            .collect();
        for function in functions.filter(|function| named.contains(&function.sig.ident.to_string())) {
            refs.collect(&function.to_token_stream().to_string(), Some(&file), None);
        }
    }
}

/// The functions a CASES list gives for a runner's name: the identifier that follows
/// the name's string and a comma.
fn runner_names(cases: &str, runner: &str) -> Vec<String> {
    let tokens = text::tokens(cases).unwrap_or_default();
    let wanted = format!("{runner:?}");
    let named = tokens.windows(3).filter_map(|window| match window {
        [name, comma, Token::Ident(function)] if name.text() == wanted && comma.text() == "," => {
            Some(function.clone())
        }
        _ => None,
    });
    named.collect()
}
