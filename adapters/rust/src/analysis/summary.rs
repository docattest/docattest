//! What a session keeps of a crate once it is read: where its modules are, what each
//! exports, and which of them the crate's users can import.

use super::modules::{Crate, Export};
use std::path::{Path, PathBuf};

/// What a session keeps of a crate once it is read. (A syntax tree cannot be shared
/// between the threads that answer requests.)
pub struct Summary {
    modules: Vec<Outline>,
}

/// What is kept of one module.
struct Outline {
    path: Vec<String>,
    file: PathBuf,
    exports: Vec<Export>,
    opaque: bool,
    /// Whether its parent declares it `pub`.
    public: bool,
}

impl Summary {
    /// Reads a crate and keeps what a session asks of it.
    pub fn of(krate: &Crate) -> Summary {
        let modules = krate.modules.iter().map(|module| Outline {
            path: module.path.clone(),
            file: module.file.clone(),
            exports: krate.exports(&module.path),
            opaque: krate.opaque(&module.path),
            public: krate.declared_public(&module.path),
        });
        Summary { modules: modules.collect() }
    }

    /// The file of the module at a path below the root.
    pub fn file_of(&self, path: &[String]) -> Option<&Path> {
        self.at(path).map(|module| module.file.as_path())
    }

    /// The path of the module a file is, when a file is one whole module.
    pub fn module_of(&self, file: &Path) -> Option<&[String]> {
        self.modules.iter().find(|module| module.file == file).map(|module| module.path.as_slice())
    }

    /// The names the module at a path exports, as its source declares them.
    pub fn exports(&self, path: &[String]) -> &[Export] {
        self.at(path).map_or(&[], |module| &module.exports)
    }

    /// Whether syntax cannot say everything the module at a path exports.
    pub fn opaque(&self, path: &[String]) -> bool {
        self.at(path).is_some_and(|module| module.opaque)
    }

    /// The first module on the way from the root to the one at a path that is not
    /// declared `pub`: nothing outside the crate imports a module through it.
    pub fn private_on(&self, path: &[String]) -> Option<Vec<String>> {
        let mut prefixes = (1..=path.len()).map(|k| &path[..k]);
        prefixes.find(|prefix| self.at(prefix).is_some_and(|module| !module.public)).map(<[String]>::to_vec)
    }

    fn at(&self, path: &[String]) -> Option<&Outline> {
        self.modules.iter().find(|module| module.path == path)
    }
}
