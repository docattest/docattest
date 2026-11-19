//! Constants read from source: a fact may read a constant a surface's module declares at
//! its top level and does not export (a `const`, or a `static` that is not `mut`), and any
//! constant of a binary crate, which nothing can import.
//!
//! The constant's declaration is written into the fact's program, with those of the other
//! constants of the module it reads, when [`literal`] says it means the same there. The
//! compiler gives it its value there: it is not taken from a build of the crate, and
//! nothing of the crate runs. A binary is never built.

mod literal;

use super::Analysis;
use super::modules::use_leaves;
use super::surfaces::Surface;
use crate::language::doc::named;
use crate::language::text::{self, Token};
use literal::{DURATION, Names};
use quote::ToTokens;
use syn::{Expr, Item, Type};

/// What a module declares that a fact may read from source, or that it names by mistake.
#[derive(Default)]
pub(crate) struct Constants {
    constants: Vec<Constant>,
    /// The `use` declarations a fact's program needs for the names the module gives std's
    /// types.
    uses: Vec<String>,
    /// What else the module declares, by name, with what it is ("a function").
    others: Vec<(String, &'static str)>,
}

/// A constant of a module, the names its initializer writes, and its declaration as a
/// fact's program writes it, or why it cannot be written there.
struct Constant {
    name: String,
    initializer: Vec<String>,
    written: Result<Written, String>,
}

struct Written {
    declaration: String,
    /// The names of the other constants of the module it reads.
    reads: Vec<String>,
}

impl Constants {
    /// Reads what a module's items declare.
    pub(crate) fn of(items: &[Item]) -> Constants {
        let duration = duration_names(items);
        let names = Names { duration: &duration };
        let mut found = Constants {
            uses: duration.iter().map(|name| format!("use ::std::time::Duration as {name};")).collect(),
            ..Constants::default()
        };
        for item in items {
            let declared = match item {
                Item::Const(c) => {
                    Declared { keyword: "const", ident: &c.ident, attrs: &c.attrs, ty: &c.ty, expr: &c.expr }
                }
                Item::Static(s) if matches!(s.mutability, syn::StaticMutability::None) => {
                    Declared { keyword: "static", ident: &s.ident, attrs: &s.attrs, ty: &s.ty, expr: &s.expr }
                }
                other => {
                    found.others.extend(named(other).map(|(name, _)| (name, kind(other))));
                    continue;
                }
            };
            found.constants.push(Constant {
                name: declared.ident.to_string(),
                initializer: written_names(declared.expr),
                written: declared.written(&names),
            });
        }
        found
    }

    /// The names of the module's constants.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.constants.iter().map(|constant| constant.name.as_str())
    }

    /// Whether the module declares a constant of a name.
    pub(crate) fn declares(&self, name: &str) -> bool {
        self.names().any(|declared| declared == name)
    }

    /// What the module declares under a name that is no constant: "a function".
    pub(crate) fn other(&self, name: &str) -> Option<&'static str> {
        self.others.iter().find(|(declared, _)| declared == name).map(|(_, what)| *what)
    }

    /// The declarations a program needs to read a constant: the `use` declarations of
    /// std's types, its own, and those of the constants it reads, at any depth, each once;
    /// or why it cannot be read. `None` when the module declares no constant of the name.
    pub(crate) fn declarations(&self, name: &str) -> Option<Result<Vec<String>, String>> {
        self.constant(name)?;
        let mut out = self.uses.clone();
        let mut pending = vec![name.to_string()];
        let mut seen: Vec<String> = Vec::new();
        while let Some(next) = pending.pop() {
            if seen.contains(&next) {
                continue;
            }
            let written = match self.constant(&next).map(|constant| &constant.written) {
                Some(Ok(written)) => written,
                Some(Err(why)) if next == name => return Some(Err(why.clone())),
                Some(Err(why)) => return Some(Err(format!("it reads {next}, and {why}"))),
                None => {
                    return Some(Err(format!("it reads `{next}`, which is not a constant of its module")));
                }
            };
            out.push(written.declaration.clone());
            pending.extend(written.reads.iter().cloned());
            seen.push(next);
        }
        Some(Ok(out))
    }

    /// The names the initializer of a constant writes, and those the initializers of the
    /// module's constants among them write, at any depth, each once, in order: what a value
    /// read holds the values of.
    fn initialized_with(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut pending = vec![name.to_string()];
        while let Some(next) = pending.pop() {
            let Some(constant) = self.constant(&next) else { continue };
            for written in &constant.initializer {
                if written != name && !out.contains(written) {
                    out.push(written.clone());
                    pending.push(written.clone());
                }
            }
        }
        out
    }

    fn constant(&self, name: &str) -> Option<&Constant> {
        self.constants.iter().find(|constant| constant.name == name)
    }
}

/// The parts of a `const` or `static` declaration.
struct Declared<'a> {
    keyword: &'static str,
    ident: &'a syn::Ident,
    attrs: &'a [syn::Attribute],
    ty: &'a Type,
    expr: &'a Expr,
}

impl Declared<'_> {
    /// The declaration as a fact's program writes it, `pub` so that the fact can import
    /// it, or why it cannot be written there.
    fn written(&self, names: &Names) -> Result<Written, String> {
        let conditional =
            self.attrs.iter().any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"));
        if conditional {
            return Err("its declaration is conditional (#[cfg])".to_string());
        }
        let mut reads = Vec::new();
        names.check_type(self.ty, &mut reads)?;
        names.check_value(self.expr, &mut reads)?;
        let (keyword, ident, ty, expr) =
            (self.keyword, self.ident, self.ty.to_token_stream(), self.expr.to_token_stream());
        Ok(Written { declaration: format!("pub {keyword} {ident}: {ty} = {expr};"), reads })
    }
}

/// What a fact's program declares to read constants from source.
#[derive(Default)]
pub(crate) struct SourceConstants {
    /// The constants the fact reads.
    pub(crate) names: Vec<String>,
    /// Their declarations, and those they need, each once.
    pub(crate) declarations: Vec<String>,
}

impl Analysis {
    /// What a fact's program declares for the constants of its surface's module it reads
    /// from source: those its expression names, the surface does not export, and the
    /// program can declare.
    pub(crate) fn source_constants(&self, surface: &Surface, expression: &str) -> SourceConstants {
        let mut found = SourceConstants::default();
        for name in text::identifiers(expression) {
            if found.names.iter().any(|known| known == name) || self.exported(surface, name) {
                continue;
            }
            let read = self.with_constants(surface, |constants| constants.declarations(name)).flatten();
            if let Some(Ok(declarations)) = read {
                found.names.push(name.to_string());
                let new: Vec<String> =
                    declarations.into_iter().filter(|line| !found.declarations.contains(line)).collect();
                found.declarations.extend(new);
            }
        }
        found
    }

    /// Whether a fact reads a name of a surface from source: its module declares a
    /// constant of that name, which the surface does not export.
    pub(crate) fn read_from_source(&self, surface: &Surface, name: &str) -> bool {
        !self.exported(surface, name)
            && self.with_constants(surface, |constants| constants.declares(name)).unwrap_or(false)
    }

    /// The names a constant of a surface's module is initialized with, at any depth through
    /// the module's other constants; none when the name is no constant of it.
    pub(crate) fn initialized_with(&self, surface: &Surface, name: &str) -> Vec<String> {
        self.with_constants(surface, |constants| constants.initialized_with(name)).unwrap_or_default()
    }

    /// Why a name a fact's program could not find is not read, when the surface's module
    /// declares it.
    pub(crate) fn unread(&self, surface: &Surface, name: &str) -> Option<String> {
        if let Some(why) = self.unread_helper(surface, name) {
            return Some(why);
        }
        let at = &surface.name;
        let why = self.with_constants(surface, |constants| match constants.declarations(name) {
            Some(Err(why)) => Some(format!(
                "{name} is a constant that surface \"{at}\" does not export, and it cannot be read from source: {why}"
            )),
            _ => constants.other(name).map(|what| {
                format!("{name} is {what} that surface \"{at}\" does not export; of its unexported names, a fact reads only constants")
            }),
        });
        why.flatten()
    }

    /// Calls a function with the constants of a surface's module, when it resolved.
    fn with_constants<T>(&self, surface: &Surface, read: impl FnOnce(&Constants) -> T) -> Option<T> {
        let located = surface.located()?;
        let summary = self.summary(located.owner.name(), located.owner.root());
        summary.constants(&located.module).map(read)
    }
}

/// The identifiers an expression writes, in order: the words of its literals are none.
fn written_names(expr: &Expr) -> Vec<String> {
    let tokens = text::tokens(&expr.to_token_stream().to_string()).unwrap_or_default();
    let names = tokens.into_iter().filter_map(|token| match token {
        Token::Ident(name) => Some(name),
        Token::Other(_) => None,
    });
    names.collect()
}

/// The names a module's `use` declarations give `std::time::Duration`.
fn duration_names(items: &[Item]) -> Vec<String> {
    let uses = items.iter().filter_map(|item| match item {
        Item::Use(used) => Some(use_leaves(&used.tree)),
        _ => None,
    });
    let is_duration = |path: &Vec<String>| {
        path.len() == DURATION.len()
            && ["std", "core"].contains(&path[0].as_str())
            && path[1..] == DURATION[1..]
    };
    uses.flatten().filter(|(_, path)| is_duration(path)).map(|(name, _)| name).collect()
}

/// What an item is, with its article, as a message names it.
fn kind(item: &Item) -> &'static str {
    match item {
        Item::Fn(_) => "a function",
        Item::Struct(_) => "a struct",
        Item::Enum(_) => "an enum",
        Item::Union(_) => "a union",
        Item::Trait(_) => "a trait",
        Item::Type(_) => "a type",
        Item::Static(_) => "a mutable static",
        Item::Mod(_) => "a module",
        Item::Macro(_) => "a macro",
        _ => "an item",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constants(source: &str) -> Constants {
        Constants::of(&syn::parse_file(source).unwrap().items)
    }

    #[test]
    fn a_constant_comes_with_those_it_reads() {
        let module = constants(
            "use std::time::Duration;\nconst BASE: u64 = 10;\nconst TIMEOUT: Duration = Duration::from_secs(BASE * 3);\nstatic NAME: &str = \"shopd\";\nstatic mut HITS: u32 = 0;\nfn port() -> u16 { 1 }",
        );
        assert_eq!(
            module.declarations("TIMEOUT"),
            Some(Ok(vec![
                "use ::std::time::Duration as Duration;".to_string(),
                "pub const TIMEOUT: Duration = Duration :: from_secs (BASE * 3);".to_string(),
                "pub const BASE: u64 = 10;".to_string(),
            ]))
        );
        assert!(module.declarations("NAME").is_some_and(|read| read.is_ok()));
        assert_eq!(module.names().collect::<Vec<_>>(), ["BASE", "TIMEOUT", "NAME"]);
        assert_eq!(
            (module.other("port"), module.other("HITS")),
            (Some("a function"), Some("a mutable static"))
        );
        assert!(module.declarations("port").is_none());
    }

    #[test]
    fn a_constant_is_initialized_with_what_its_initializer_names_at_any_depth() {
        let module = constants(
            "const ENV: &[&str] = &[PATH, SHELL];\nconst PATH: &str = \"/bin/run\";\nconst SHELL: &str = DEFAULT;\nconst DEFAULT: &str = \"sh\";\nconst NEXT: u8 = width() + 1;\nfn width() -> u8 { LIMIT }\nconst LIMIT: u8 = 2;",
        );
        assert_eq!(module.initialized_with("ENV"), ["PATH", "SHELL", "DEFAULT"]);
        assert_eq!(module.initialized_with("NEXT"), ["width"]);
        assert!(module.initialized_with("width").is_empty());
    }

    #[test]
    fn a_constant_that_reads_one_that_cannot_be_read_says_so() {
        let module = constants(
            "const LEVEL: Level = Level(2);\nconst NEXT: u8 = LEVEL.0 + 1;\nconst LOOSE: u8 = MISSING;\n#[cfg(unix)]\nconst SEP: char = '/';",
        );
        let why = |name: &str| module.declarations(name).and_then(Result::err);
        assert_eq!(
            why("NEXT").as_deref(),
            Some("it reads LEVEL, and its type `Level` is not made of primitive types and Duration")
        );
        assert_eq!(
            why("LOOSE").as_deref(),
            Some("it reads `MISSING`, which is not a constant of its module")
        );
        assert_eq!(why("SEP").as_deref(), Some("its declaration is conditional (#[cfg])"));
    }
}
