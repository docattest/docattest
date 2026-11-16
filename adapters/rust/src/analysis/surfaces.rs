//! Surfaces: each is one module of a crate of the project.
//!
//! An entry of the config is a module path (`shop`, `shop::shipping`), a path of the
//! project (a package's directory, or the file of a module), or both, when the module is
//! imported under one path and read from a file that path does not lead to.

use super::Analysis;
use super::documented::Listing;
use super::modules::Export;
use crate::project::Package;
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
    /// The package whose library holds it; none when the surface did not resolve.
    pub(crate) package: Option<Package>,
    /// The module's path below its crate's root.
    pub(crate) module: Vec<String>,
    /// The module's file; none when the surface did not resolve.
    pub(crate) file: Option<PathBuf>,
    /// The path the config gave, when it gave one.
    pub(crate) written: Option<String>,
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

/// The module path an entry names, when it names one.
fn written_import(entry: &Value) -> Option<String> {
    let import = entry
        .get("import")
        .and_then(Value::as_str)
        .or_else(|| entry.as_str().filter(|text| !is_path(text)))?;
    Some(import.to_string())
}

impl Analysis {
    pub(super) fn resolve(&self, configured: &Configured) -> Surface {
        let (written, import) = (written_path(&configured.entry), written_import(&configured.entry));
        let mut surface = Surface {
            name: configured.name.clone(),
            import: import.clone().unwrap_or_default(),
            package: None,
            module: Vec::new(),
            file: None,
            written: written.clone(),
        };
        let found = match (&written, &import) {
            (Some(path), _) => self.module_at(path),
            (None, Some(import)) => self.module_named(import),
            (None, None) => None,
        };
        if let Some((package, module, file)) = found {
            if surface.import.is_empty() {
                surface.import = std::iter::once(package.lib.clone())
                    .chain(module.iter().cloned())
                    .collect::<Vec<_>>()
                    .join("::");
            }
            (surface.package, surface.module, surface.file) = (Some(package), module, Some(file));
        }
        surface
    }

    /// The module at a path of the project: a package's root for its directory, else the
    /// module a file is.
    fn module_at(&self, path: &str) -> Option<(Package, Vec<String>, PathBuf)> {
        let place = self.abs(path).canonicalize().ok()?;
        for package in &self.project.packages {
            if place == package.dir {
                return Some((package.clone(), Vec::new(), package.root.clone()));
            }
            if let Some(module) = self.krate(package).module_of(&place) {
                return Some((package.clone(), module.to_vec(), place));
            }
        }
        None
    }

    /// The module a module path names, in a crate of the project.
    fn module_named(&self, import: &str) -> Option<(Package, Vec<String>, PathBuf)> {
        let mut segments = import.split("::").map(str::to_string);
        let package = self.project.by_lib(&segments.next()?)?;
        let path: Vec<String> = segments.collect();
        let file = self.krate(package).file_of(&path)?.to_path_buf();
        Some((package.clone(), path, file))
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
    /// source does not declare all of it and the compiler can.
    pub(crate) fn exports(&self, surface: &Surface) -> Option<Vec<Export>> {
        let package = surface.package.as_ref()?;
        let compiled = match self.listing(package, &surface.module) {
            Listing::Compiled(documented) => documented.exports(&surface.module),
            Listing::Read | Listing::Refused(_) => None,
        };
        Some(compiled.unwrap_or_else(|| self.krate(package).exports(&surface.module).to_vec()))
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
        let (Some(package), Some(file)) = (&surface.package, &surface.file) else {
            return json!({ "warnings": [self.unresolved(surface)] });
        };
        let warnings: Vec<String> = match self.listing(package, &surface.module) {
            Listing::Refused(why) => vec![unlisted(surface, &why)],
            Listing::Read | Listing::Compiled(_) => Vec::new(),
        };
        json!({
            "resolution": {
                "import": surface.import,
                "entry": self.rel(file),
                "package": { "name": package.name, "display": package.name },
            },
            "warnings": warnings,
        })
    }

    /// Why a surface has no module.
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
        surface.package.is_none().then(|| format!("surface \"{name}\" ({shown}) not found"))
    }
}

/// What is said of a surface whose exports the compiler was asked for and could not list.
fn unlisted(surface: &Surface, why: &str) -> String {
    let import = &surface.import;
    format!(
        "macros write some of what \"{import}\" exports, and the compiler could not list it ({why}); only the exports its source declares are known"
    )
}
