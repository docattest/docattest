//! Cargo, as the adapter uses it: what a project is made of, and building the crate the
//! adapter generates beside it.

use super::{Command, run};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The cargo to run: `DOCATTEST_CARGO`, else `cargo` on the `PATH`.
pub fn program() -> PathBuf {
    std::env::var_os("DOCATTEST_CARGO").map_or_else(|| "cargo".into(), PathBuf::from)
}

/// The variable that names the features the generated crate is built with, as cargo's
/// `--features` takes them (`shop/eu shop/tax`). A crate's features are cargo's to choose,
/// and the protocol carries nothing of a language's own settings.
const FEATURES_VARIABLE: &str = "DOCATTEST_CARGO_FEATURES";

/// The features of the project's crates the generated crate is built with, when
/// [`FEATURES_VARIABLE`] names any.
pub fn features() -> Option<String> {
    std::env::var(FEATURES_VARIABLE).ok().filter(|features| !features.is_empty())
}

/// The cfg every build of the generated crate sets on every crate, so that the items of a
/// project under `#[cfg(docattest)]`, its docs helpers, exist in docattest's builds and in
/// no other. One configuration for every program keeps one build of the dependencies.
pub const DOCS_CFG: &str = "docattest";

/// Where a build adds `--cfg` [`DOCS_CFG`]. Cargo takes a crate's flags from the first of
/// `CARGO_ENCODED_RUSTFLAGS`, `RUSTFLAGS` and its config that is set, so the cfg joins the
/// variable that is set, else the config's `build.rustflags`, which cargo joins with those
/// of the project's own config: the flags the project builds with are kept.
#[derive(Debug, PartialEq)]
enum DocsCfg {
    /// A variable, with the flags it holds and the cfg.
    Variable(&'static str, String),
    /// A `--config` value.
    Config(String),
}

/// Where a build adds the cfg, given what the two variables hold.
fn docs_cfg(encoded: Option<String>, spaced: Option<String>) -> DocsCfg {
    let joined = |held: String, separator: &str| {
        let added = ["--cfg", DOCS_CFG].join(separator);
        if held.is_empty() { added } else { format!("{held}{separator}{added}") }
    };
    match (encoded, spaced) {
        (Some(held), _) => DocsCfg::Variable("CARGO_ENCODED_RUSTFLAGS", joined(held, "\x1f")),
        (None, Some(held)) => DocsCfg::Variable("RUSTFLAGS", joined(held, " ")),
        (None, None) => DocsCfg::Config(format!("build.rustflags=[\"--cfg\", \"{DOCS_CFG}\"]")),
    }
}

/// `cargo metadata` of a workspace, as far as the adapter reads it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub target_directory: PathBuf,
    pub workspace_root: PathBuf,
}

/// One package of a workspace.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Package {
    pub name: String,
    pub manifest_path: PathBuf,
    pub edition: String,
    pub targets: Vec<Target>,
    pub dependencies: Vec<Dependency>,
}

/// One target of a package: its library, a program, a test.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub src_path: PathBuf,
}

/// One dependency a package declares.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub req: String,
    pub kind: Option<String>,
    pub rename: Option<String>,
    pub optional: bool,
    pub uses_default_features: bool,
    pub features: Vec<String>,
    pub target: Option<String>,
    pub path: Option<PathBuf>,
    pub source: Option<String>,
}

/// Asks cargo what the workspace at or above a directory is made of.
pub fn metadata(dir: &Path) -> Result<Metadata, String> {
    let args = ["metadata", "--format-version", "1", "--no-deps"].map(String::from).to_vec();
    let asked = run(&Command { program: program(), args, dir: dir.to_path_buf(), ..Command::default() });
    if let Some(error) = asked.error {
        return Err(not_run(&error));
    }
    if asked.code != Some(0) {
        return Err(last_error(&asked.stderr));
    }
    serde_json::from_str(&asked.stdout).map_err(|err| format!("cargo metadata: {err}"))
}

/// Why cargo did not start, naming the program tried, which may be `DOCATTEST_CARGO`'s.
pub fn not_run(error: &str) -> String {
    format!("{} could not be run: {error}", program().display())
}

/// The last thing cargo said went wrong, on one line.
pub fn last_error(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().filter(|line| !line.trim().is_empty()).collect();
    let said = lines.iter().rev().find(|line| line.starts_with("error")).or(lines.last());
    said.map_or_else(|| "cargo failed".to_string(), |line| line.trim().to_string())
}

/// One complaint of a build.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    /// The program the complaint is about, by its name; empty when it is about none.
    pub target: String,
    /// The file of the complaint, as cargo names it; empty for a complaint about no file.
    pub file: String,
    pub line: usize,
    pub message: String,
}

#[derive(Deserialize)]
struct Message {
    reason: String,
    target: Option<Target>,
    message: Option<Diagnostic>,
}

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    message: String,
    spans: Vec<Span>,
}

#[derive(Deserialize)]
struct Span {
    file_name: String,
    line_start: usize,
    is_primary: bool,
    /// Where the macro was used, when the span is inside what a macro wrote.
    expansion: Option<Box<Expansion>>,
}

#[derive(Deserialize)]
struct Expansion {
    span: Span,
}

impl Span {
    /// The place in the code as it was written: where the outermost macro was used.
    fn written(&self) -> &Span {
        self.expansion.as_ref().map_or(self, |expansion| expansion.span.written())
    }
}

/// Builds programs of a crate, by name, into a target directory, with some features, and
/// reads what the compiler complains of. A program that does not compile does not stop
/// the others. The problems are empty exactly when every program was built. Every crate
/// is built with the cfg [`DOCS_CFG`].
pub fn build(krate: &Path, target: &Path, programs: &[String], features: Option<String>) -> Vec<Problem> {
    let mut args: Vec<String> =
        ["build", "--keep-going", "--message-format=json", "--target-dir"].map(String::from).to_vec();
    args.push(target.to_string_lossy().into_owned());
    let mut env = Vec::new();
    let variable = |name: &str| std::env::var(name).ok();
    match docs_cfg(variable("CARGO_ENCODED_RUSTFLAGS"), variable("RUSTFLAGS")) {
        DocsCfg::Variable(name, flags) => env.push((name.to_string(), flags)),
        DocsCfg::Config(value) => args.extend(["--config".to_string(), value]),
    }
    if let Some(features) = features {
        args.extend(["--features".to_string(), features]);
    }
    for name in programs {
        args.extend(["--bin".to_string(), name.clone()]);
    }
    let built =
        run(&Command { program: program(), args, dir: krate.to_path_buf(), env, ..Command::default() });
    if let Some(error) = built.error {
        return vec![Problem {
            target: String::new(),
            file: String::new(),
            line: 0,
            message: not_run(&error),
        }];
    }
    let mut problems = errors(&built.stdout);
    if built.code != Some(0) && problems.is_empty() {
        problems.push(Problem {
            target: String::new(),
            file: String::new(),
            line: 0,
            message: last_error(&built.stderr),
        });
    }
    problems
}

/// The compiler's errors among the messages of a build.
pub fn errors(messages: &str) -> Vec<Problem> {
    let mut problems = Vec::new();
    for line in messages.lines() {
        let Ok(message) = serde_json::from_str::<Message>(line) else { continue };
        let Some(diagnostic) =
            message.message.filter(|d| message.reason == "compiler-message" && d.level == "error")
        else {
            continue;
        };
        // "aborting due to 2 previous errors" is about the errors, and at no place.
        let Some(span) = diagnostic.spans.iter().find(|span| span.is_primary).map(Span::written) else {
            continue;
        };
        problems.push(Problem {
            target: message.target.map(|t| t.name).unwrap_or_default(),
            file: span.file_name.clone(),
            line: span.line_start,
            message: diagnostic.message,
        });
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_read_from_the_compilers_messages() {
        let messages = concat!(
            r#"{"reason":"compiler-message","target":{"name":"e2","kind":["bin"],"src_path":"/x/src/bin/e2.rs"},"message":{"level":"error","message":"cannot find value `NO` in this scope","spans":[{"file_name":"src/bin/e2.rs","line_start":3,"is_primary":true}]}}"#,
            "\n",
            r#"{"reason":"compiler-message","target":{"name":"e2","kind":["bin"],"src_path":"/x"},"message":{"level":"warning","message":"unused","spans":[]}}"#,
            "\n",
            r#"{"reason":"compiler-message","target":{"name":"e2","kind":["bin"],"src_path":"/x"},"message":{"level":"error","message":"aborting due to 1 previous error","spans":[]}}"#,
            "\nnot json\n",
            r#"{"reason":"build-finished","success":false}"#,
        );
        let want = Problem {
            target: "e2".into(),
            file: "src/bin/e2.rs".into(),
            line: 3,
            message: "cannot find value `NO` in this scope".into(),
        };
        assert_eq!(errors(messages), vec![want]);
        assert_eq!(last_error("warning: x\nerror: no such package\n\n"), "error: no such package");
    }

    #[test]
    fn the_docs_cfg_joins_the_flags_the_project_builds_with() {
        let held = |text: &str| Some(text.to_string());
        let variable = |name, flags: &str| DocsCfg::Variable(name, flags.to_string());
        assert_eq!(
            docs_cfg(None, None),
            DocsCfg::Config("build.rustflags=[\"--cfg\", \"docattest\"]".to_string())
        );
        assert_eq!(docs_cfg(None, held("-D warnings")), variable("RUSTFLAGS", "-D warnings --cfg docattest"));
        assert_eq!(docs_cfg(None, held("")), variable("RUSTFLAGS", "--cfg docattest"));
        assert_eq!(
            docs_cfg(held("-Dwarnings"), held("ignored")),
            variable("CARGO_ENCODED_RUSTFLAGS", "-Dwarnings\x1f--cfg\x1fdocattest")
        );
    }
}
