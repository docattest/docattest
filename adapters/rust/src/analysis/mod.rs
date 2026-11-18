//! What is read from a project's syntax, with nothing run: its surfaces and what they
//! export, the symbols an attestation names, the tests a file declares, what a change
//! affects, and the setup module's names. Nothing is compiled either, but for the one
//! thing syntax cannot say: what a module exports when a macro writes some of it
//! ([`documented`]).

mod affected;
mod constants;
mod documented;
pub(crate) mod modules;
mod setup;
mod summary;
pub(crate) mod surfaces;
mod symbols;
mod tests;

use crate::project::{Package, Project};
use documented::Documented;
use modules::Crate;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use summary::Summary;
use surfaces::{Configured, Surface};

/// A project as its syntax shows it, for one docattest invocation.
pub(crate) struct Analysis {
    /// The project's directory.
    pub(crate) root: PathBuf,
    pub(crate) project: Project,
    /// The setup module's path in the project, when the config names one.
    pub(crate) setup: Option<String>,
    surfaces: Vec<Surface>,
    /// Each crate read, by its root file: a library and a binary may share a name.
    crates: Mutex<HashMap<PathBuf, Arc<Summary>>>,
    /// Each library the compiler was asked to list, or why it could not.
    documented: Mutex<HashMap<String, Result<Arc<Documented>, String>>>,
}

impl Analysis {
    /// Reads a project's configured surfaces from its workspace.
    pub(crate) fn new(
        root: PathBuf,
        project: Project,
        setup: Option<String>,
        configured: &[Configured],
    ) -> Self {
        let mut analysis = Analysis {
            root,
            project,
            setup,
            surfaces: Vec::new(),
            crates: Mutex::default(),
            documented: Mutex::default(),
        };
        analysis.surfaces = configured.iter().map(|configured| analysis.resolve(configured)).collect();
        analysis
    }

    /// The absolute path of a project path.
    pub(crate) fn abs(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// A path relative to the project root with `/` separators, or the path itself when
    /// it is outside the project.
    pub(crate) fn rel(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => path.to_string_lossy().into_owned(),
        }
    }

    /// A package's library, as far as a session asks about it: read once.
    fn krate(&self, package: &Package) -> Arc<Summary> {
        self.summary(&package.lib, &package.root)
    }

    /// A crate, by its name and root file, as far as a session asks about it: read once.
    fn summary(&self, name: &str, root: &Path) -> Arc<Summary> {
        let mut crates = self.crates.lock().unwrap_or_else(|p| p.into_inner());
        let read = |path: &Path| std::fs::read_to_string(path).ok();
        let loaded = crates
            .entry(root.to_path_buf())
            .or_insert_with(|| Arc::new(Summary::of(&Crate::load(name, root, &read))));
        Arc::clone(loaded)
    }

    /// Which of some names still occur as an identifier in the project's code: the files
    /// given, and the files of the surfaces' modules.
    pub(crate) fn in_code(&self, names: &[String], files: &[String]) -> Vec<String> {
        let mut paths: Vec<PathBuf> = files.iter().map(|file| self.abs(file)).collect();
        paths.extend(self.surfaces.iter().filter_map(|surface| surface.file().map(Path::to_path_buf)));
        let texts: Vec<String> = paths.iter().filter_map(|path| std::fs::read_to_string(path).ok()).collect();
        let held = |name: &String| {
            texts.iter().any(|text| crate::language::text::identifiers(text).contains(&name.as_str()))
        };
        names.iter().filter(|name| held(name)).cloned().collect()
    }
}
