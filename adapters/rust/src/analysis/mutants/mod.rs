//! `mutants`: edits to the literals of the declarations of the symbols a proof reads, which
//! `mutate` applies one at a time to see whether the proof can fail.
//!
//! A symbol of a surface is the item its module declares under the name, or the one a
//! `pub use` of the module leads to, at any depth; a symbol without a surface is a
//! constant a fact reads from source, of the first surface whose module declares one of
//! that name. A constant or a static is changed where its value is written
//! ([`literals`]); any other item is computed when it runs, and has nothing to change.
//! A name declared under `#[cfg]` more than once is changed in each declaration: syntax
//! cannot tell which one is built.

mod literals;
mod offsets;

use super::Analysis;
use super::modules::Crate;
use super::surfaces::Surface;
use crate::language::doc::named;
use crate::protocol::{AskedSymbol, MutantEdit, NoMutants, SymbolMutants};
use offsets::Offsets;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use syn::{Expr, Item, Type};

impl Analysis {
    /// Answers `mutants`: for each symbol in order, at most `limit` edits, the first in
    /// source order, or why there are none.
    pub(crate) fn mutants(&self, symbols: &[AskedSymbol], limit: usize) -> Vec<SymbolMutants> {
        let mut sources = HashMap::new();
        symbols.iter().map(|symbol| self.symbol_mutants(symbol, limit, &mut sources)).collect()
    }

    /// The edits of one symbol. `sources` holds each crate read so far, by its root file.
    fn symbol_mutants(
        &self,
        symbol: &AskedSymbol,
        limit: usize,
        sources: &mut HashMap<PathBuf, Source>,
    ) -> SymbolMutants {
        let Some(located) = self.declaring_surface(symbol).and_then(Surface::located) else {
            return SymbolMutants::none(NoMutants::NotFound);
        };
        let (name, root) = (located.owner.name(), located.owner.root());
        let source = sources.entry(root.to_path_buf()).or_insert_with(|| Source::load(name, root));
        let (at, declared) = source.krate.origin(&located.module, &symbol.name);
        let Some(module) = source.krate.module(&at) else {
            return SymbolMutants::none(NoMutants::NotFound);
        };
        match declaration(&module.items, &declared) {
            Declaration::Values(values) => {
                let text = &source.texts[&module.file];
                let edits = self.edits(&module.file, text, &values, limit);
                SymbolMutants::new(edits, NoMutants::NoLiteral)
            }
            Declaration::Computed => SymbolMutants::none(NoMutants::Function),
            Declaration::Missing => SymbolMutants::none(NoMutants::NotFound),
        }
    }

    /// The surface whose module a symbol is looked for in.
    fn declaring_surface(&self, symbol: &AskedSymbol) -> Option<&Surface> {
        match &symbol.surface {
            Some(name) => self.surface(name),
            None => self.surfaces.iter().find(|surface| self.read_from_source(surface, &symbol.name)),
        }
    }

    /// The first `limit` edits of the literals of some values declared in a file.
    fn edits(&self, file: &Path, text: &str, values: &[(&Type, &Expr)], limit: usize) -> Vec<MutantEdit> {
        let offsets = Offsets::new(text);
        let changes = values.iter().flat_map(|(ty, value)| literals::changes(value, ty, &offsets));
        let path = self.rel(file);
        let edit = |change: literals::Change| MutantEdit {
            path: path.clone(),
            start: change.range.start,
            end: change.range.end,
            replacement: change.replacement,
            description: change.description,
        };
        changes.take(limit).map(edit).collect()
    }
}

/// A crate, read again for its syntax trees (a session keeps none: their spans belong to
/// the thread that parsed them), with the text of each file it was read from.
struct Source {
    krate: Crate,
    texts: HashMap<PathBuf, String>,
}

impl Source {
    fn load(name: &str, root: &Path) -> Source {
        let texts = RefCell::new(HashMap::new());
        let read = |path: &Path| {
            let text = std::fs::read_to_string(path).ok()?;
            texts.borrow_mut().insert(path.to_path_buf(), text.clone());
            Some(text)
        };
        let krate = Crate::load(name, root, &read);
        Source { krate, texts: texts.into_inner() }
    }
}

/// What a module declares under a name.
enum Declaration<'a> {
    /// Constants or statics, each with its type and its value.
    Values(Vec<(&'a Type, &'a Expr)>),
    /// An item whose value is computed when it runs: a function, a type.
    Computed,
    Missing,
}

fn declaration<'a>(items: &'a [Item], name: &str) -> Declaration<'a> {
    let values: Vec<(&Type, &Expr)> = items
        .iter()
        .filter_map(|item| match item {
            Item::Const(constant) if constant.ident == name => Some((&*constant.ty, &*constant.expr)),
            Item::Static(variable) if variable.ident == name => Some((&*variable.ty, &*variable.expr)),
            _ => None,
        })
        .collect();
    if !values.is_empty() {
        return Declaration::Values(values);
    }
    if items.iter().any(|item| named(item).is_some_and(|(declared, _)| declared == name)) {
        return Declaration::Computed;
    }
    Declaration::Missing
}
