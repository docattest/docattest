//! `affected`: the surface exports a change to the code affects.
//!
//! A crate is cut into units, one for each name a module declares; what an `impl` block
//! holds is part of its type's unit. A unit's fingerprint is its tokens without its doc
//! comments, so reformatting, a comment or a doc edit changes nothing. An export is
//! affected when the unit it refers to changed, appeared or disappeared, or reaches such
//! a unit through the paths it writes, in the working tree or at the base. It is read
//! from syntax, which errs towards too much: a path stands for any unit of that name, a
//! method is found through its type, and a trait is not followed to what implements it.
//! A constant a fact reads from source, and a docs helper, is listed as an export is,
//! since a fact attests it.
//!
//! What a macro writes has no declaration to read. So the macro calls of a module are
//! one unit, and a name that nothing declares stands for it: an export only the
//! compiler lists, and a path that leads to no declaration in a module that calls macros.
//! Every item of such a module writes a name of that kind, so a change to a module's
//! macro calls affects all the module declares.

use super::Analysis;
use super::constants::Constants;
use super::docs_helpers::docs_helpers;
use super::modules::{Crate, Module, calls_macro, use_leaves};
use super::surfaces::Located;
use crate::language::doc::{impl_owner, named};
use crate::language::text;
use crate::protocol::SymbolRef;
use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use syn::Item;

/// A unit: a module's path below its crate's root, and a name it declares.
type Key = (Vec<String>, String);

/// The name of the unit that holds a module's macro calls: no item can have it.
const BY_MACROS: &str = "!";

/// The code behind one name.
#[derive(Default)]
struct Unit {
    fingerprint: String,
    uses: Vec<Key>,
}

/// A token stream without the doc comments in it, at any depth.
fn without_docs(stream: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = stream.into_iter().collect();
    let is_doc = |tree: Option<&TokenTree>| match tree {
        Some(TokenTree::Group(group)) => {
            matches!(group.stream().into_iter().next(), Some(TokenTree::Ident(i)) if i == "doc")
        }
        _ => false,
    };
    let mut out = Vec::new();
    let mut k = 0;
    while k < trees.len() {
        match &trees[k] {
            TokenTree::Punct(p) if p.as_char() == '#' && is_doc(trees.get(k + 1)) => k += 1,
            TokenTree::Group(group) => {
                out.push(TokenTree::Group(proc_macro2::Group::new(
                    group.delimiter(),
                    without_docs(group.stream()),
                )));
            }
            other => out.push(other.clone()),
        }
        k += 1;
    }
    out.into_iter().collect()
}

/// The name whose unit an item is part of: its own, its type's for an `impl` block, or
/// the one of its module's macro calls.
pub(super) fn owner(item: &Item) -> Option<String> {
    match item {
        Item::Impl(block) => impl_owner(block),
        call if calls_macro(call) => Some(BY_MACROS.to_string()),
        Item::Use(_) | Item::Mod(_) => None,
        other => named(other).map(|(name, _)| name),
    }
}

/// The units of a crate, by key.
fn units(krate: &Crate) -> HashMap<Key, Unit> {
    let mut out: HashMap<Key, Unit> = HashMap::new();
    for module in &krate.modules {
        let imports = imports(module);
        for item in &module.items {
            let Some(name) = owner(item) else { continue };
            let tokens = without_docs(item.to_token_stream()).to_string();
            let unit = out.entry((module.path.clone(), name)).or_default();
            unit.uses.extend(
                text::paths(&tokens).iter().filter_map(|path| resolve(krate, module, &imports, path)),
            );
            unit.fingerprint.push_str(&tokens);
            unit.fingerprint.push('\n');
        }
    }
    out
}

/// What a module's `use` declarations bring in.
struct Imports {
    /// Each name brought in, with the path it stands for.
    named: HashMap<String, Vec<String>>,
    /// The path of each module brought in whole.
    whole: Vec<Vec<String>>,
}

fn imports(module: &Module) -> Imports {
    let uses = module.items.iter().filter_map(|item| match item {
        Item::Use(used) => Some(use_leaves(&used.tree)),
        _ => None,
    });
    let (whole, named): (Vec<_>, Vec<_>) = uses.flatten().partition(|(name, _)| name == "*");
    Imports { named: named.into_iter().collect(), whole: whole.into_iter().map(|(_, path)| path).collect() }
}

/// The unit a path written in a module stands for. The path is read as what the name it
/// begins with was imported as, else as it is written, else as what a glob brings in; a
/// declaration is looked for before what a macro may have written.
fn resolve(krate: &Crate, module: &Module, imports: &Imports, written: &[String]) -> Option<Key> {
    let below = |prefix: &[String], rest: &[String]| prefix.iter().chain(rest).cloned().collect::<Vec<_>>();
    // A glob of a module brings in what is below it; a glob of an enum, its variants,
    // which stand for the enum.
    let brought = |whole: &Vec<String>| match krate.whole(&module.path, whole) {
        Some(_) => Some(below(whole, written)),
        None => has_variant(krate, module, whole, &written[0]).then(|| whole.clone()),
    };
    let readings: Vec<Vec<String>> = match imports.named.get(&written[0]) {
        Some(stands_for) => vec![below(stands_for, &written[1..])],
        None => std::iter::once(written.to_vec()).chain(imports.whole.iter().filter_map(brought)).collect(),
    };
    let declared = readings.iter().find_map(|full| declared(krate, module, full));
    declared.or_else(|| readings.iter().find_map(|full| by_macros(krate, module, full)))
}

/// Whether a path written in a module names an enum of the crate that has a variant.
fn has_variant(krate: &Crate, module: &Module, path: &[String], variant: &str) -> bool {
    let Some((at, name)) = krate.resolve(&module.path, path) else { return false };
    let is_it = |item: &Item| match item {
        Item::Enum(found) => found.ident == name && found.variants.iter().any(|held| held.ident == variant),
        _ => false,
    };
    krate.module(&at).is_some_and(|module| module.items.iter().any(is_it))
}

/// The unit a full path stands for: the longest beginning of it that names a declaration
/// of the crate (`Money::new` is `Money`).
fn declared(krate: &Crate, module: &Module, full: &[String]) -> Option<Key> {
    (1..=full.len()).rev().find_map(|len| {
        let (at, name) = krate.resolve(&module.path, &full[..len])?;
        let exported = krate.exports(&at).iter().any(|export| export.name == name);
        (exported || declares(krate, &at, &name)).then(|| krate.origin(&at, &name))
    })
}

/// The macro calls of the module a full path leads into, when it calls any: they may
/// have written the name the path goes on with.
fn by_macros(krate: &Crate, module: &Module, full: &[String]) -> Option<Key> {
    (1..=full.len()).rev().find_map(|len| {
        let (at, _) = krate.resolve(&module.path, &full[..len])?;
        declares(krate, &at, BY_MACROS).then(|| (at, BY_MACROS.to_string()))
    })
}

/// Whether a module has a unit of a name.
fn declares(krate: &Crate, module: &[String], name: &str) -> bool {
    krate
        .module(module)
        .is_some_and(|module| module.items.iter().any(|item| owner(item).as_deref() == Some(name)))
}

/// The units that are in a set, or reach one of it through what they use.
fn reaching(units: &HashMap<Key, Unit>, set: &HashSet<Key>) -> HashSet<Key> {
    let mut found = set.clone();
    loop {
        let grown: Vec<Key> = units
            .iter()
            .filter(|(key, unit)| !found.contains(*key) && unit.uses.iter().any(|used| found.contains(used)))
            .map(|(key, _)| key.clone())
            .collect();
        if grown.is_empty() {
            return found;
        }
        found.extend(grown);
    }
}

impl Analysis {
    /// Answers `affected`. `base` holds the text of every changed file as it was at the
    /// base; `None` for a file the base did not have.
    pub(crate) fn affected(&self, base: &HashMap<String, Option<String>>) -> Vec<SymbolRef> {
        let was: HashMap<_, _> = base.iter().map(|(rel, text)| (self.abs(rel), text.clone())).collect();
        let read_now = |path: &Path| std::fs::read_to_string(path).ok();
        let read_was = |path: &Path| was.get(path).cloned().unwrap_or_else(|| read_now(path));
        let mut out: Vec<SymbolRef> = Vec::new();
        let mut crates: HashMap<PathBuf, (Crate, Crate, Changes)> = HashMap::new();
        for surface in &self.surfaces {
            let Some(Located { owner, module, .. }) = surface.located() else {
                continue;
            };
            let (name, root) = (owner.name(), owner.root());
            let (now, before, changes) = crates.entry(root.to_path_buf()).or_insert_with(|| {
                let (now, before) = (Crate::load(name, root, &read_now), Crate::load(name, root, &read_was));
                let changes = Changes::between(&now, &before);
                (now, before, changes)
            });
            // What the compiler lists of the working tree, then what each revision's
            // source declares: the base cannot be compiled.
            let listed = self.exports(surface).unwrap_or_default();
            let mut exports: Vec<(String, Key)> =
                listed.into_iter().filter_map(|export| Some((export.name, export.origin?))).collect();
            for krate in [&*now, &*before] {
                let read = krate.exports(module).into_iter();
                exports.extend(read.map(|export| {
                    let unit = krate.origin(module, &export.name);
                    (export.name, unit)
                }));
                exports.extend(attested_beyond_exports(krate, module));
            }
            let from = out.len();
            for (name, unit) in exports {
                if changes.touch(&unit) && !out[from..].iter().any(|found| found.name == name) {
                    out.push(SymbolRef { name, surface: surface.name.clone() });
                }
            }
        }
        out
    }
}

/// What a fact attests of a module that it does not export, each its own unit: the
/// constants it may read from source, and its docs helpers.
fn attested_beyond_exports(krate: &Crate, module: &[String]) -> Vec<(String, Key)> {
    let Some(items) = krate.module(module).map(|module| &module.items) else { return Vec::new() };
    let constants = Constants::of(items).names().map(str::to_string).collect::<Vec<_>>();
    let helpers = docs_helpers(items).into_iter().map(|helper| helper.name);
    constants.into_iter().chain(helpers).map(|name| (name.clone(), (module.to_vec(), name))).collect()
}

/// What a change did to a crate's units.
struct Changes {
    /// The units that differ between the two revisions, or are in only one, and those
    /// that reach them in either.
    touched: HashSet<Key>,
    /// Every unit either revision declares.
    declared: HashSet<Key>,
}

impl Changes {
    fn between(now: &Crate, before: &Crate) -> Changes {
        let (units_now, units_before) = (units(now), units(before));
        let mut differing: HashSet<Key> = HashSet::new();
        for (key, unit) in &units_now {
            if units_before.get(key).is_none_or(|was| was.fingerprint != unit.fingerprint) {
                differing.insert(key.clone());
            }
        }
        differing.extend(units_before.keys().filter(|key| !units_now.contains_key(*key)).cloned());
        let mut touched = reaching(&units_now, &differing);
        touched.extend(reaching(&units_before, &differing));
        Changes { touched, declared: units_now.into_keys().chain(units_before.into_keys()).collect() }
    }

    /// Whether the change touches the unit behind a name: the name's own when a revision
    /// declares it, else the macro calls of its module, which must have written it.
    fn touch(&self, unit: &Key) -> bool {
        if self.declared.contains(unit) {
            return self.touched.contains(unit);
        }
        self.touched.contains(&(unit.0.clone(), BY_MACROS.to_string()))
    }
}
