//! What a project is made of: the packages of its cargo workspace that have a library.

use crate::toolchain::cargo::{self, Dependency};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// One package of the workspace that has a library.
#[derive(Clone, Debug)]
pub(crate) struct Package {
    /// The name the library is imported by.
    pub(crate) lib: String,
    /// The package's name, as a dependency names it.
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
    /// The library's root file.
    pub(crate) root: PathBuf,
    pub(crate) edition: String,
    pub(crate) dependencies: Vec<Dependency>,
}

/// One binary crate of the workspace (`src/main.rs`, `src/bin/shopd.rs`). Nothing can
/// import it: a fact reads only its constants, from source.
#[derive(Clone, Debug)]
pub(crate) struct Binary {
    /// The binary's name, as `cargo run --bin` takes it.
    pub(crate) name: String,
    /// The directory of its package.
    pub(crate) dir: PathBuf,
    /// The binary's root file.
    pub(crate) root: PathBuf,
}

/// A target of a workspace package that `cargo test` builds: its library, a program, a
/// test, a bench or an example.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    /// Its package's name, as `cargo --package` takes it.
    pub(crate) package: String,
    /// Its name, as `cargo --bin`, `--test`, `--bench` and `--example` take it.
    pub(crate) name: String,
    pub(crate) kind: TargetKind,
    /// Its root file.
    pub(crate) root: PathBuf,
}

/// What a target is, which decides how `cargo test` selects it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TargetKind {
    Lib,
    Bin,
    Test,
    Bench,
    Example,
}

/// A project's cargo workspace.
#[derive(Clone, Debug, Default)]
pub(crate) struct Project {
    pub(crate) packages: Vec<Package>,
    pub(crate) binaries: Vec<Binary>,
    pub(crate) targets: Vec<Target>,
    /// The workspace's root, where its manifest and lock file are.
    pub(crate) workspace: PathBuf,
    /// Where the adapter's own crate is generated and built: beside the project's builds,
    /// in a directory of this workspace's own.
    pub(crate) work: PathBuf,
    /// Why cargo could not say what the project is made of.
    pub(crate) error: Option<String>,
}

impl Project {
    /// Asks cargo about the workspace a directory is in. `in_memory` is the root a project
    /// that exists only in memory stands for, when it is one.
    pub(crate) fn read(start: &Path, in_memory: Option<&str>) -> Project {
        let metadata = match cargo::metadata(start) {
            Ok(metadata) => metadata,
            Err(error) => return Project { error: Some(error), ..Project::default() },
        };
        let packages = metadata.packages.iter().filter_map(package).collect();
        let binaries = metadata.packages.iter().flat_map(binaries).collect();
        let targets = metadata.packages.iter().flat_map(targets).collect();
        // A project in memory is written somewhere new each time; its builds are kept in
        // one place for the project it stands for, so that they are not started over. A
        // workspace keeps its own below its target directory, named for its root: copies
        // of a project may share one target directory, linked, and two of them must never
        // write one generated crate with different path dependencies at once.
        let (builds, owner) = match in_memory {
            Some(root) => (std::env::temp_dir().join("docattest-rust"), root.to_string()),
            None => (
                metadata.target_directory.join("docattest"),
                metadata.workspace_root.to_string_lossy().into_owned(),
            ),
        };
        let work = builds.join(format!("{:016x}", hash(&owner)));
        Project { packages, binaries, targets, workspace: metadata.workspace_root, work, error: None }
    }

    /// The package whose library is imported by a name.
    pub(crate) fn by_lib(&self, lib: &str) -> Option<&Package> {
        self.packages.iter().find(|package| package.lib == lib)
    }
}

fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// A package of cargo's as one of ours, when it has a library.
fn package(found: &cargo::Package) -> Option<Package> {
    let lib =
        found.targets.iter().find(|target| target.kind.iter().any(|kind| kind == "lib" || kind == "rlib"))?;
    Some(Package {
        lib: lib.name.replace('-', "_"),
        name: found.name.clone(),
        dir: found.manifest_path.parent()?.to_path_buf(),
        root: lib.src_path.clone(),
        edition: found.edition.clone(),
        dependencies: found.dependencies.clone(),
    })
}

/// The binaries of a package of cargo's.
fn binaries(found: &cargo::Package) -> Vec<Binary> {
    let dir = found.manifest_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let programs = found.targets.iter().filter(|target| target.kind.iter().any(|kind| kind == "bin"));
    programs
        .map(|target| Binary { name: target.name.clone(), dir: dir.clone(), root: target.src_path.clone() })
        .collect()
}

/// The targets of a package of cargo's that `cargo test` builds: all but its build script.
fn targets(found: &cargo::Package) -> Vec<Target> {
    let ours = |target: &cargo::Target| {
        Some(Target {
            package: found.name.clone(),
            name: target.name.clone(),
            kind: target.kind.iter().find_map(|kind| target_kind(kind))?,
            root: target.src_path.clone(),
        })
    };
    found.targets.iter().filter_map(ours).collect()
}

/// A kind cargo metadata gives a target as ours: a library's are its crate types.
fn target_kind(kind: &str) -> Option<TargetKind> {
    Some(match kind {
        "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro" => TargetKind::Lib,
        "bin" => TargetKind::Bin,
        "test" => TargetKind::Test,
        "bench" => TargetKind::Bench,
        "example" => TargetKind::Example,
        _ => return None,
    })
}

/// Writes a project that exists only in memory to a directory: cargo reads files.
pub(crate) fn materialise(named: &str, files: &HashMap<String, String>) -> std::io::Result<PathBuf> {
    let unique = format!(
        "docattest-rust-{}-{:016x}",
        std::process::id(),
        hash(&format!("{named}{:?}", std::time::Instant::now()))
    );
    let root = std::env::temp_dir().join(unique);
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or(&root))?;
        std::fs::write(path, text)?;
    }
    std::fs::create_dir_all(&root)?;
    root.canonicalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn copies_of_a_project_that_share_a_target_directory_build_apart() {
        let base = std::env::temp_dir().join(format!("docattest-rust-work-{}", std::process::id()));
        let (original, copy) = (base.join("shop"), base.join("shop-copy"));
        for root in [&original, &copy] {
            std::fs::create_dir_all(root.join("src")).unwrap();
            std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"shop\"\nversion = \"0.1.0\"\n")
                .unwrap();
            std::fs::write(root.join("src/lib.rs"), "").unwrap();
        }
        std::fs::create_dir_all(original.join("target")).unwrap();
        std::os::unix::fs::symlink(original.join("target"), copy.join("target")).unwrap();
        let works = [&original, &copy].map(|root| {
            let work = Project::read(root, None).work;
            std::fs::create_dir_all(&work).unwrap();
            work.canonicalize().unwrap()
        });
        let shared = original.join("target/docattest").canonicalize().unwrap();
        let _ = std::fs::remove_dir_all(&base);
        assert_ne!(works[0], works[1]);
        assert!(works.iter().all(|work| work.parent() == Some(shared.as_path())), "{works:?}");
    }
}
