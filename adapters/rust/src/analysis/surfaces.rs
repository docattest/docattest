//! Surfaces: each is one module of a crate of the project.
//!
//! An entry of the config is a module path (`shop`, `shop::shipping`), a path of the
//! project (a package's directory, or the file of a module), or both, when the module is
//! imported under one path and read from a file that path does not lead to. A binary
//! crate, which nothing imports, is named by a path: its root file (`./src/main.rs`), a
//! module's file below it, or the directory of a package that has no library and one
//! binary.

use super::Analysis;
use super::documented::Listing;
use super::modules::Export;
use crate::project::{Binary, Package};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// A surface as the config wrote it.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Configured {
    pub(crate) name: String,
    pub(crate) entry: Value,
}

/// One configured surface.
pub(crate) struct Surface {
    pub(crate) name: String,
    /// The path examples import it by (`shop::shipping`); empty when it cannot be told.
    pub(crate) import: String,
    /// The path the config gave, when it gave one.
    pub(crate) written: Option<String>,
    pub(crate) resolution: Resolution,
}

/// What a surface's entry resolved to.
pub(crate) enum Resolution {
    /// A module examples can import.
    Located(Located),
    /// A module nothing outside its crate can import, because one on its import path is
    /// private: why.
    Private(String),
    /// Nothing.
    Unresolved,
}

/// Where a resolved surface's module is.
pub(crate) struct Located {
    /// The crate that holds it.
    pub(crate) owner: Owner,
    /// The module's path below its crate's root.
    pub(crate) module: Vec<String>,
    /// The module's file.
    pub(crate) file: PathBuf,
}

/// The crate a surface's module is of.
pub(crate) enum Owner {
    /// A package's library, which examples and facts import.
    Library(Package),
    /// A binary, of which facts read only constants, from source.
    Binary(Binary),
}

impl Owner {
    /// The crate's name: what its library is imported by, or the binary's name.
    pub(crate) fn name(&self) -> &str {
        match self {
            Owner::Library(package) => &package.lib,
            Owner::Binary(binary) => &binary.name,
        }
    }

    /// The crate's root file.
    pub(crate) fn root(&self) -> &Path {
        match self {
            Owner::Library(package) => &package.root,
            Owner::Binary(binary) => &binary.root,
        }
    }
}

impl Surface {
    /// Where its module is; none when the surface did not resolve.
    pub(crate) fn located(&self) -> Option<&Located> {
        match &self.resolution {
            Resolution::Located(located) => Some(located),
            Resolution::Private(_) | Resolution::Unresolved => None,
        }
    }

    /// Its module's file; none when the surface did not resolve.
    pub(crate) fn file(&self) -> Option<&Path> {
        self.located().map(|located| located.file.as_path())
    }

    /// Whether its module is of a library, which a fact's program imports.
    pub(crate) fn importable(&self) -> bool {
        self.located().is_some_and(|located| matches!(located.owner, Owner::Library(_)))
    }
}

/// The path of the project an entry names, when it names one and not a module path.
fn written_path(entry: &Value) -> Option<String> {
    let path =
        entry.get("path").and_then(Value::as_str).or_else(|| entry.as_str().filter(|text| is_path(text)))?;
    Some(path.to_string())
}

/// The directory whose workspace the project is: that of the first surface that names a
/// place, else the project root.
pub(crate) fn workspace_start(root: &Path, configured: &[Configured]) -> PathBuf {
    configured
        .iter()
        .filter_map(|configured| written_path(&configured.entry))
        .map(|path| root.join(path))
        .map(
            |path| {
                if path.is_dir() { path } else { path.parent().map(Path::to_path_buf).unwrap_or_default() }
            },
        )
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| root.to_path_buf())
}

/// Whether an entry names a place in the project and not a module path.
fn is_path(written: &str) -> bool {
    written == "." || written.contains('/') || written.ends_with(".rs")
}

/// The module path an entry names, when it names one. A path written from the root
/// (`::shop`) is the same path: the programs that import it name it from the root.
fn written_import(entry: &Value) -> Option<String> {
    let import = entry
        .get("import")
        .and_then(Value::as_str)
        .or_else(|| entry.as_str().filter(|text| !is_path(text)))?;
    Some(import.trim_start_matches("::").to_string())
}

impl Analysis {
    pub(super) fn resolve(&self, configured: &Configured) -> Surface {
        let (written, import) = (written_path(&configured.entry), written_import(&configured.entry));
        let found = match (&written, &import) {
            (Some(path), _) => self.module_at(path),
            (None, Some(import)) => self.module_named(import),
            (None, None) => None,
        };
        let import = match (import.filter(|import| !import.is_empty()), &found) {
            (Some(import), _) => import,
            (None, Some(located)) => import_of(located),
            (None, None) => String::new(),
        };
        let private = match &found {
            Some(Located { owner: Owner::Library(_), .. }) => self.private_on(&import),
            _ => None,
        };
        let resolution = match (found, private) {
            (None, _) => Resolution::Unresolved,
            (Some(_), Some(why)) => Resolution::Private(why),
            (Some(located), None) => Resolution::Located(located),
        };
        Surface { name: configured.name.clone(), import, written, resolution }
    }

    /// Why an import path cannot be imported from outside its crate, when a module it goes
    /// through is declared without `pub`. A path that is not made of the crate's module
    /// declarations (a `pub use` gives a module another name) is not judged.
    fn private_on(&self, import: &str) -> Option<String> {
        let mut segments = import.split("::");
        let package = self.project.by_lib(segments.next()?)?;
        let path: Vec<String> = segments.map(str::to_string).collect();
        let krate = self.krate(package);
        let private = krate.private_on(&path)?;
        let declared_in = krate.file_of(&private[..private.len() - 1]).map(|file| self.rel(file));
        Some(format!(
            "\"{import}\" cannot be imported from outside its crate: module `{}` is private (declared without `pub` in {}); name a public module, or the one that re-exports what it declares",
            private.join("::"),
            declared_in.unwrap_or_default()
        ))
    }

    /// The module at a path of the project: a package's root for its directory, else the
    /// module a file is. A library is looked for before a binary.
    fn module_at(&self, path: &str) -> Option<Located> {
        let place = self.abs(path).canonicalize().ok()?;
        let libraries = self.project.packages.iter().map(|package| Owner::Library(package.clone()));
        let binaries = self.project.binaries.iter().map(|binary| Owner::Binary(binary.clone()));
        libraries.chain(binaries).find_map(|owner| self.module_of(owner, &place))
    }

    /// The module of a crate at a place: its root, for the directory it is the crate of,
    /// or the module a file is.
    fn module_of(&self, owner: Owner, place: &Path) -> Option<Located> {
        if self.crate_of_directory(&owner, place) {
            let file = owner.root().to_path_buf();
            return Some(Located { owner, module: Vec::new(), file });
        }
        let module = self.summary(owner.name(), owner.root()).module_of(place)?.to_vec();
        Some(Located { owner, module, file: place.to_path_buf() })
    }

    /// Whether a crate is the one a directory names: a package's library, or the one
    /// binary of a package that has no library.
    fn crate_of_directory(&self, owner: &Owner, place: &Path) -> bool {
        match owner {
            Owner::Library(package) => package.dir == place,
            Owner::Binary(binary) => {
                let of_package = |dir: &PathBuf| dir == place;
                binary.dir == place
                    && !self.project.packages.iter().any(|package| of_package(&package.dir))
                    && self.project.binaries.iter().filter(|other| of_package(&other.dir)).count() == 1
            }
        }
    }

    /// The module a module path names, in a crate of the project.
    fn module_named(&self, import: &str) -> Option<Located> {
        let mut segments = import.split("::").map(str::to_string);
        let package = self.project.by_lib(&segments.next()?)?;
        let module: Vec<String> = segments.collect();
        let file = self.krate(package).file_of(&module)?.to_path_buf();
        Some(Located { owner: Owner::Library(package.clone()), module, file })
    }

    /// A configured surface by name.
    pub(crate) fn surface(&self, name: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|surface| surface.name == name)
    }

    /// The names a surface exports; `None` when it did not resolve.
    pub(crate) fn exports_of(&self, name: &str) -> Option<Vec<String>> {
        let surface = self.surface(name)?;
        Some(self.exports(surface)?.into_iter().map(|export| export.name).collect())
    }

    /// What a surface exports: read from its source, or listed by the compiler when its
    /// source does not declare all of it and the compiler can. A binary exports nothing.
    pub(crate) fn exports(&self, surface: &Surface) -> Option<Vec<Export>> {
        let Located { owner, module, .. } = surface.located()?;
        let Owner::Library(package) = owner else { return Some(Vec::new()) };
        let compiled = match self.listing(package, module) {
            Listing::Compiled(documented) => documented.exports(module),
            Listing::Read | Listing::Refused(_) => None,
        };
        Some(compiled.unwrap_or_else(|| self.krate(package).exports(module).to_vec()))
    }

    /// Whether a surface exports a name.
    pub(crate) fn exported(&self, surface: &Surface, name: &str) -> bool {
        self.exports(surface).is_some_and(|exports| exports.iter().any(|export| export.name == name))
    }

    pub(crate) fn describe_surfaces(&self) -> Value {
        Value::Object(
            self.surfaces.iter().map(|surface| (surface.name.clone(), self.describe(surface))).collect(),
        )
    }

    fn describe(&self, surface: &Surface) -> Value {
        let Located { owner, module, file } = match &surface.resolution {
            Resolution::Located(located) => located,
            Resolution::Private(why) => {
                return json!({ "warnings": [format!("{why}; examples and facts that use it fail")] });
            }
            Resolution::Unresolved => return json!({ "warnings": [self.unresolved(surface)] }),
        };
        let (package, warnings) = match owner {
            Owner::Library(package) => match self.listing(package, module) {
                Listing::Refused(why) => (&package.name, vec![unlisted(surface, &why)]),
                Listing::Read | Listing::Compiled(_) => (&package.name, Vec::new()),
            },
            Owner::Binary(binary) => (&binary.name, Vec::new()),
        };
        json!({
            "resolution": {
                "import": surface.import,
                "entry": self.rel(file),
                "package": { "name": package, "display": package },
            },
            "warnings": warnings,
        })
    }

    /// Why a surface's entry resolved to nothing.
    fn unresolved(&self, surface: &Surface) -> String {
        let (what, why) = match (&surface.written, &self.project.error) {
            (_, Some(error)) => {
                (surface.import.clone(), format!("cargo could not read the project: {error}"))
            }
            (Some(path), None) => {
                (path.clone(), "no module of a library of the workspace is there".to_string())
            }
            (None, None) => {
                (surface.import.clone(), "no library of the workspace has that module".to_string())
            }
        };
        let what = if what.is_empty() { surface.written.clone().unwrap_or_default() } else { what };
        format!("\"{what}\" did not resolve ({why}); examples and facts that use it fail")
    }

    /// Why a fact or a walk of a surface cannot go on, or `None` when it can.
    pub(crate) fn not_found(&self, name: &str) -> Option<String> {
        let Some(surface) = self.surface(name) else {
            let names: Vec<&str> = self.surfaces.iter().map(|surface| surface.name.as_str()).collect();
            return Some(format!("unknown surface \"{name}\" (configured: {})", names.join(", ")));
        };
        let shown = if surface.import.is_empty() {
            surface.written.clone().unwrap_or_default()
        } else {
            surface.import.clone()
        };
        match &surface.resolution {
            Resolution::Located(_) => None,
            Resolution::Private(why) => Some(format!("surface \"{name}\": {why}")),
            Resolution::Unresolved => Some(format!("surface \"{name}\" ({shown}) not found")),
        }
    }
}

/// The path examples import a located module by; a binary's, by the binary's name.
fn import_of(located: &Located) -> String {
    std::iter::once(located.owner.name().to_string())
        .chain(located.module.iter().cloned())
        .collect::<Vec<_>>()
        .join("::")
}

/// What is said of a surface whose exports the compiler was asked for and could not list.
fn unlisted(surface: &Surface, why: &str) -> String {
    let import = &surface.import;
    format!(
        "macros write some of what \"{import}\" exports, and the compiler could not list it ({why}); only the exports its source declares are known"
    )
}
