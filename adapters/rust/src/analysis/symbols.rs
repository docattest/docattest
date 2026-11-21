//! `symbols`: the surface symbols an attestation names.
//!
//! Read from tokens, so that what a macro is given counts (`assert_eq!(shop::total(…))`):
//! a path that goes through a surface's module names the export after it, a `use` makes
//! a shorter name stand for such a path, and a glob import of a surface's module, or
//! code written in that module's own file, reaches its exports by their bare names. A
//! cases runner, and a function of the setup module or a docs helper that a fact calls,
//! also names what the items of its own file that it calls name ([`reached`]). A constant
//! a fact or a runner reads holds the values its initializer uses, so it names those
//! items of its module too, at any depth through other constants; a function it calls is
//! named, and its body is not followed.

use super::Analysis;
use super::modules::use_leaves;
use super::reach::reached;
use super::surfaces::Surface;
use super::tests::{cited, declared};
use crate::language::doc::named;
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
    /// Which names of a surface are collected: its exports, or its docs helpers.
    collected: &'a dyn Fn(&Surface, &str) -> bool,
    found: Vec<SymbolRef>,
}

impl Refs<'_> {
    /// Records a name of a surface, when it is one of those collected.
    fn add(&mut self, name: &str, surface: &Surface) {
        if (self.collected)(surface, name) {
            self.record(name, surface);
        }
    }

    /// Records a name a value of a surface is read through: an export, or a constant of
    /// its module read from source, which no one can import.
    fn read(&mut self, name: &str, surface: &Surface) {
        if self.analysis.read_from_source(surface, name) {
            self.record(name, surface);
        } else {
            self.add(name, surface);
        }
    }

    /// Records a name of a surface, once.
    fn record(&mut self, name: &str, surface: &Surface) {
        let found = SymbolRef { name: name.to_string(), surface: surface.name.clone() };
        if !self.found.contains(&found) {
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
                let bare = (own.is_some() && surface.file() == own) || uses.imports_whole(&import);
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
        let exported = |surface: &Surface, name: &str| self.exported(surface, name);
        let mut refs = Refs { analysis: self, collected: &exported, found: Vec::new() };
        match attestation.kind.as_str() {
            "example" => {
                let own = is_source(&attestation.page).then(|| self.abs(&attestation.page));
                if let Some(owner) = attestation.owner.as_ref().filter(|_| own.is_some()) {
                    for surface in self.surfaces.iter().filter(|surface| surface.file() == own.as_deref()) {
                        refs.add(owner, surface);
                    }
                }
                refs.collect(
                    &blocks::visible(&attestation.code),
                    blocks::parsed(&attestation.code).as_ref(),
                    None,
                );
            }
            "fact" => self.fact_symbols(&mut refs, attestation),
            "test" => self.citation_symbols(&mut refs, attestation),
            "cases" => self.runner_symbols(&mut refs, &attestation.runner),
            _ => {}
        }
        refs.found
    }

    /// The docs helpers an example's code names, read as `symbols` reads what it names.
    /// An example is built with them, but runs as the project's users build its crates,
    /// without them, so naming one is a mistake.
    pub(crate) fn docs_helpers_named(&self, code: &str) -> Vec<SymbolRef> {
        let helper = |surface: &Surface, name: &str| self.docs_helper(surface, name).is_some();
        let mut refs = Refs { analysis: self, collected: &helper, found: Vec::new() };
        refs.collect(&blocks::visible(code), blocks::parsed(code).as_ref(), None);
        refs.found
    }

    /// What the test a citation names uses, following local non-test helper items in the
    /// enclosing module and file.
    fn citation_symbols(&self, refs: &mut Refs, attestation: &Attestation) {
        let path = self.abs(&attestation.file);
        let Some(file) = std::fs::read_to_string(&path).ok().and_then(|text| syn::parse_file(&text).ok())
        else {
            return;
        };
        let all = declared(&file);
        let Some(test) = cited(&all, &attestation.title, &attestation.describes) else { return };
        let Some(function) = test.function else { return };
        let code = function.to_token_stream().to_string();
        refs.collect(&code, Some(&file), Some(&path));
        let test_names: Vec<&str> =
            all.iter().filter(|t| t.declared.kind.is_none()).map(|t| t.declared.title.as_str()).collect();
        let followed = |name: &str| !test_names.contains(&name);
        let roots: Vec<String> = text::identifiers(&code).into_iter().map(str::to_string).collect();
        for scope in helper_scopes(&file.items, &test.declared.describes, self.is_surface_file(&path)) {
            for item in reached(scope, &roots, &followed) {
                refs.collect(&item.to_token_stream().to_string(), Some(&file), Some(&path));
            }
        }
    }

    /// Whether `path` is the file of any configured surface (so its top-level items are
    /// production code, not test helpers).
    fn is_surface_file(&self, path: &std::path::Path) -> bool {
        self.surfaces.iter().any(|s| s.file() == Some(path))
    }

    /// What a fact names: the exports and constants its expression reads, what the docs
    /// helpers and the setup module's functions it calls reach, and the paths it writes.
    fn fact_symbols(&self, refs: &mut Refs, fact: &Attestation) {
        let names = text::identifiers(&fact.expression);
        if let Some(surface) = self.surface(&fact.surface) {
            for &name in &names {
                refs.read(name, surface);
            }
            self.helper_symbols(refs, surface, &names);
        }
        let setup = self.setup_names();
        let called: Vec<String> = names
            .into_iter()
            .filter(|name| setup.iter().any(|known| known == name))
            .map(str::to_string)
            .collect();
        self.setup_symbols(refs, &called);
        refs.collect(&fact.expression, None, None);
        self.initializer_symbols(refs);
    }

    /// What the docs helpers a fact calls reach through the other docs helpers and the
    /// unexported constants of their module: the fact attests each of those, the exports
    /// they use, and the functions of the module they call. A docs helper is glue, so its
    /// body is followed; an ordinary function's is not, as that of a function a fact calls
    /// is not.
    fn helper_symbols(&self, refs: &mut Refs, surface: &Surface, names: &[&str]) {
        let called: Vec<String> = names
            .iter()
            .filter(|name| self.callable_helper(surface, name))
            .map(|name| name.to_string())
            .collect();
        if called.is_empty() {
            return;
        }
        let Some(items) = self.module_items(surface) else { return };
        let module = syn::File { shebang: None, attrs: Vec::new(), items };
        let followed =
            |name: &str| self.read_from_source(surface, name) || self.docs_helper(surface, name).is_some();
        let functions: Vec<String> = module.items.iter().filter_map(function_name).collect();
        for item in reached(&module.items, &called, &followed) {
            if let Some((name, _)) = named(item) {
                refs.record(&name, surface);
            }
            let code = item.to_token_stream().to_string();
            for name in called_names(&code).iter().filter(|name| functions.contains(name)) {
                refs.record(name, surface);
            }
            refs.collect(&code, Some(&module), surface.file());
        }
    }

    /// What some items of the setup module reach: their own code, and that of the setup
    /// module's items it names, at any depth.
    fn setup_symbols(&self, refs: &mut Refs, roots: &[String]) {
        if roots.is_empty() {
            return;
        }
        let Some(file) = self.setup_file() else { return };
        for item in reached(&file.items, roots, &|_| true) {
            refs.collect(&item.to_token_stream().to_string(), Some(&file), None);
        }
    }

    /// What the runner of a cases table uses: the function the setup module's CASES
    /// gives for its name, and what it reaches.
    fn runner_symbols(&self, refs: &mut Refs, runner: &str) {
        let Some(file) = self.setup_file() else { return };
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
        self.setup_symbols(refs, &named);
        self.initializer_symbols(refs);
    }

    /// What the constants found so far are initialized with: the exports and constants of
    /// their module that their initializers name, at any depth through other constants.
    fn initializer_symbols(&self, refs: &mut Refs) {
        let found = refs.found.clone();
        for symbol in &found {
            let Some(surface) = self.surface(&symbol.surface) else { continue };
            for name in self.initialized_with(surface, &symbol.name) {
                refs.read(&name, surface);
            }
        }
    }
}

/// The item slices where a test's local helper items may live: the enclosing inline test
/// module(s) when the test is inside a `mod`, plus the file's top-level items only when the
/// file is a dedicated test file (not a surface's production source file).
fn helper_scopes<'a>(
    items: &'a [syn::Item],
    describes: &[String],
    in_surface_file: bool,
) -> Vec<&'a [syn::Item]> {
    let mut scopes = Vec::new();
    if !in_surface_file {
        scopes.push(items);
    }
    let mut current = items;
    for name in describes {
        let Some(next) = current.iter().find_map(|item| match item {
            syn::Item::Mod(module) if module.ident == name => {
                module.content.as_ref().map(|(_, list)| list.as_slice())
            }
            _ => None,
        }) else {
            break;
        };
        scopes.push(next);
        current = next;
    }
    scopes
}

/// The name of an item that is a function.
fn function_name(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
        _ => None,
    }
}

/// The names some code calls: each identifier written right before `(`.
fn called_names(code: &str) -> Vec<String> {
    let tokens = text::tokens(code).unwrap_or_default();
    let calls = tokens.windows(2).filter_map(|pair| match pair {
        [Token::Ident(name), open] if open.text() == "(" => Some(name.clone()),
        _ => None,
    });
    calls.collect()
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
