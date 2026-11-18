//! The crate the adapter generates beside the project, and never inside it: every
//! example, and every program that reads facts or runs a table, is one of its programs.
//!
//! It depends on the project's libraries by path, and on what they depend on, so that a
//! block may use what a doc test of the project may use. It carries the project's own
//! `[patch]` tables and starts from its lock file, so the project's code is built from
//! the same sources as in the project's own build. It lives in the project's target
//! directory (`target/docattest`), with a target directory of its own, and what cargo
//! built there is still there the next time.

use super::Runner;
use crate::toolchain::cargo::{self, Dependency, Problem};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// The name of the generated crate.
pub(super) const CRATE: &str = "docattest_blocks";

/// The files of the generated crate's `harness` module, by name: the modules of this
/// crate's own `harness`, as text.
const HARNESS: [(&str, &str); 7] = [
    ("mod.rs", include_str!("../harness/mod.rs")),
    ("cells/mod.rs", include_str!("../harness/cells/mod.rs")),
    ("cells/links.rs", include_str!("../harness/cells/links.rs")),
    ("cells/raw.rs", include_str!("../harness/cells/raw.rs")),
    ("json.rs", include_str!("../harness/json.rs")),
    ("observe.rs", include_str!("../harness/observe.rs")),
    ("serve.rs", include_str!("../harness/serve.rs")),
];

/// Writes a file when its text is not already what it holds: cargo builds again what
/// was written again.
fn write_if_changed(path: &Path, text: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|held| held == text) {
        return Ok(());
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    std::fs::create_dir_all(parent)
        .and_then(|()| std::fs::write(path, text))
        .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// A name for a program: the same text is the same program, in every run.
pub(super) fn program_name(prefix: &str, source: &str) -> String {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{prefix}_{:016x}", hasher.finish())
}

/// A string as TOML writes one.
fn quoted(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}

/// A dependency of a package as a line of the generated manifest; `None` for one a
/// block cannot be given (it is optional, for a build script, or for one platform).
fn dependency_line(dependency: &Dependency) -> Option<(String, String)> {
    if dependency.optional || dependency.target.is_some() || dependency.kind.as_deref() == Some("build") {
        return None;
    }
    let key = dependency.rename.clone().unwrap_or_else(|| dependency.name.clone());
    let mut fields = Vec::new();
    match (&dependency.path, dependency.source.as_deref().and_then(|source| source.strip_prefix("git+"))) {
        (Some(path), _) => fields.push(format!("path = {}", quoted(&path.to_string_lossy()))),
        (None, Some(git)) => fields.extend(git_fields(git)),
        (None, None) => fields.push(format!("version = {}", quoted(&dependency.req))),
    }
    if dependency.rename.is_some() {
        fields.push(format!("package = {}", quoted(&dependency.name)));
    }
    if !dependency.uses_default_features {
        fields.push("default-features = false".to_string());
    }
    if !dependency.features.is_empty() {
        let features: Vec<String> = dependency.features.iter().map(|feature| quoted(feature)).collect();
        fields.push(format!("features = [{}]", features.join(", ")));
    }
    Some((key.clone(), format!("{} = {{ {} }}", quoted(&key), fields.join(", "))))
}

/// A git source as cargo metadata writes it (`url?branch=x#commit`) as manifest fields.
fn git_fields(source: &str) -> Vec<String> {
    let source = source.split('#').next().unwrap_or(source);
    let (url, query) = source.split_once('?').unwrap_or((source, ""));
    let mut fields = vec![format!("git = {}", quoted(url))];
    for (key, value) in query.split('&').filter_map(|pair| pair.split_once('=')) {
        fields.push(format!("{key} = {}", quoted(value)));
    }
    fields
}

/// The `[patch]` tables of a workspace's manifest, with their paths made absolute.
fn patches(workspace: &Path) -> Result<String, String> {
    let path = workspace.join("Cargo.toml");
    let manifest =
        std::fs::read_to_string(&path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let table =
        manifest.parse::<toml::Table>().map_err(|err| format!("cannot read {}: {err}", path.display()));
    let Some(toml::Value::Table(mut patch)) = table?.remove("patch") else {
        return Ok(String::new());
    };
    for (_, registry) in patch.iter_mut() {
        for (_, entry) in registry.as_table_mut().into_iter().flatten() {
            if let Some(path) =
                entry.get("path").and_then(toml::Value::as_str).map(|path| workspace.join(path))
            {
                entry
                    .as_table_mut()
                    .map(|table| table.insert("path".into(), path.to_string_lossy().into_owned().into()));
            }
        }
    }
    toml::to_string(&toml::Table::from_iter([("patch".to_string(), toml::Value::Table(patch))]))
        .map_err(|err| format!("cannot write the project's [patch] tables: {err}"))
}

impl Runner {
    fn crate_dir(&self) -> PathBuf {
        self.analysis.project.work.join("crate")
    }

    /// The manifest of the generated crate.
    fn manifest(&self) -> Result<String, String> {
        let packages = &self.analysis.project.packages;
        let edition = packages.first().map_or("2021", |package| package.edition.as_str());
        let mut lines: Vec<String> = packages
            .iter()
            .map(|p| format!("{} = {{ path = {} }}", quoted(&p.name), quoted(&p.dir.to_string_lossy())))
            .collect();
        let mut taken: Vec<String> = packages.iter().map(|package| package.name.clone()).collect();
        for (key, line) in
            packages.iter().flat_map(|package| &package.dependencies).filter_map(dependency_line)
        {
            if !taken.contains(&key) {
                taken.push(key);
                lines.push(line);
            }
        }
        Ok(format!(
            "[package]\nname = \"{CRATE}\"\nversion = \"0.0.0\"\nedition = \"{edition}\"\npublish = false\n\n\
             [dependencies]\n{}\n\n[profile.dev]\ndebug = false\n\n[workspace]\n\n{}",
            lines.join("\n"),
            patches(&self.analysis.project.workspace)?
        ))
    }

    /// Writes what every program of the generated crate needs: its manifest, the
    /// project's lock file to start from, and the harness. Without a workspace there is
    /// nowhere to write them.
    fn prepare(&self) -> Result<(), String> {
        if let Some(error) = &self.analysis.project.error {
            return Err(format!("cargo could not read the project: {error}"));
        }
        let dir = self.crate_dir();
        write_if_changed(&dir.join("Cargo.toml"), &self.manifest()?)?;
        write_if_changed(&dir.join("src/lib.rs"), "pub mod harness;\npub use harness::cells;\n")?;
        for (name, text) in HARNESS {
            write_if_changed(&dir.join("src/harness").join(name), text)?;
        }
        let (locked, ours) = (self.analysis.project.workspace.join("Cargo.lock"), dir.join("Cargo.lock"));
        let modified = |path: &Path| std::fs::metadata(path).and_then(|held| held.modified()).ok();
        if modified(&locked).is_some() && modified(&locked) > modified(&ours) {
            std::fs::copy(&locked, &ours)
                .map_err(|err| format!("cannot copy {} to {}: {err}", locked.display(), ours.display()))?;
        }
        Ok(())
    }

    /// Writes programs of the generated crate, each given as its name and its source, and
    /// builds them, one build at a time. A file that cannot be written is a problem about
    /// every program.
    pub(super) fn build(&self, programs: &[(String, String)]) -> Vec<Problem> {
        let _one = self.one_build();
        let written = self.prepare().and_then(|()| {
            programs.iter().try_for_each(|(name, source)| {
                write_if_changed(&self.crate_dir().join("src/bin").join(format!("{name}.rs")), source)
            })
        });
        if let Err(message) = written {
            return vec![Problem { target: String::new(), file: String::new(), line: 0, message }];
        }
        let names: Vec<String> = programs.iter().map(|(name, _)| name.clone()).collect();
        let problems = cargo::build(&self.crate_dir(), &self.analysis.project.work.join("target"), &names);
        problems.into_iter().map(|problem| self.located(problem)).collect()
    }

    /// A problem with its file named from the project's root, where a reader finds it:
    /// cargo names the generated crate's own files from that crate's directory.
    fn located(&self, problem: Problem) -> Problem {
        if problem.file.is_empty() {
            return problem;
        }
        let file = self.analysis.rel(&self.crate_dir().join(&problem.file));
        Problem { file, ..problem }
    }

    /// Where a program of the generated crate is, once built.
    pub(super) fn binary(&self, name: &str) -> PathBuf {
        let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        self.analysis.project.work.join("target").join("debug").join(file)
    }

    /// A new directory of the session's own, under the project's work.
    pub(super) fn scratch(&self, name: &str) -> Result<PathBuf, String> {
        let dir = self.analysis.project.work.join("jobs").join(name);
        std::fs::create_dir_all(&dir).map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
        Ok(dir)
    }
}

/// The line of a program a problem of its build is at; `None` when the problem is about
/// something else (the project's own code, another program).
pub(super) fn line_in(problem: &Problem, name: &str) -> Option<usize> {
    (problem.target == name && problem.file.ends_with(&format!("{name}.rs"))).then_some(problem.line)
}

/// A problem that is about no one program, as one line: it is then about all of them.
pub(super) fn general(problem: &Problem) -> String {
    if problem.file.is_empty() {
        return problem.message.clone();
    }
    format!("{}:{}: {}", problem.file, problem.line, problem.message)
}
