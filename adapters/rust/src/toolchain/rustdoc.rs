//! rustdoc's JSON: a library as the compiler sees it, after every macro has written what
//! it writes.
//!
//! The output is unstable, so a stable toolchain gives it only when `RUSTC_BOOTSTRAP`
//! names the crate; it names that one crate, and the crate's dependencies are built as
//! they always are. A toolchain that refuses is an error the caller can do without.

use super::cargo::{last_error, program};
use super::{Command, run};
use std::path::{Path, PathBuf};

/// A library to document.
pub struct Library<'a> {
    /// The package's name, as cargo names it.
    pub package: &'a str,
    /// The name the library is imported by.
    pub name: &'a str,
    /// The package's directory.
    pub dir: &'a Path,
}

/// Documents a library as JSON, private items too, and gives the text. `target` is
/// where cargo builds.
pub fn document(library: &Library, target: &Path) -> Result<String, String> {
    let args = [
        "rustdoc",
        "--package",
        library.package,
        "--lib",
        "--target-dir",
        &target.to_string_lossy(),
        "--",
        "-Z",
        "unstable-options",
        "--output-format",
        "json",
        "--document-private-items",
    ];
    let documented = run(&Command {
        program: program(),
        args: args.map(String::from).to_vec(),
        dir: library.dir.to_path_buf(),
        env: vec![("RUSTC_BOOTSTRAP".to_string(), library.name.to_string())],
        ..Command::default()
    });
    if let Some(error) = documented.error {
        return Err(format!("cargo could not be run: {error}"));
    }
    if documented.code != Some(0) {
        // The compiler's first complaint is the cause; cargo's last line only says it failed.
        let first = documented.stderr.lines().find(|line| line.starts_with("error"));
        return Err(first.map_or_else(|| last_error(&documented.stderr), |line| line.trim().to_string()));
    }
    let written = written(target, library.name)
        .ok_or_else(|| format!("rustdoc wrote no {}.json below {}", library.name, target.display()))?;
    std::fs::read_to_string(&written).map_err(|err| format!("{}: {err}", written.display()))
}

/// The file rustdoc wrote: in the target directory's `doc`, or in that of the platform
/// the project is built for, when its cargo configuration names one.
fn written(target: &Path, library: &str) -> Option<PathBuf> {
    let below = std::fs::read_dir(target).ok()?.filter_map(|entry| Some(entry.ok()?.path()));
    std::iter::once(target.to_path_buf())
        .chain(below)
        .map(|dir| dir.join("doc").join(format!("{library}.json")))
        .find(|file| file.is_file())
}
