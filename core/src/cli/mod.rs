//! The command line: one function per command (`src/cli/`).
//!
//! Exit codes: 0, 1 for failing checks and for mistakes in the request or the project (a
//! message, no stack), 2 for a bug in docattest (or the adapter), printed with what is known.

mod agents;
pub(crate) mod args;
mod audit;
mod check;
mod ci_reports;
pub mod command;
mod conformance;
mod explain;
mod git_hook;
mod hook;
pub(crate) mod hooks;
mod impact;
mod init;
mod refresh;
pub(crate) mod skill;
pub(crate) mod usage;

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::api::errors::{Error, Result};
use crate::config::DEFAULT_SURFACE;
use crate::files::source::DiskFiles;
use crate::json::pretty;
use crate::project::check::Network;
use crate::project::check::{Reference, build_index, symbols_only, uncovered as uncovered_names};
use crate::project::state::{State, load_state};
use crate::report::Detail;
use crate::snapshots::now_ms;
use args::{Options, parse_command_line};
use usage::USAGE;

/// What every command is given: the words after it, the project root and the options.
struct Invocation<'a> {
    args: &'a [String],
    root: &'a Path,
    options: &'a Options,
}

type Command = fn(&Invocation, &mut Io) -> Result<i32>;

/// The subcommands, by name.
const COMMANDS: [(&str, Command); 9] = [
    ("init", |run, io| init::init(run.root, run.options, io)),
    ("check", check_command),
    ("audit", |run, io| audit::audit(run.args, run.root, run.options, io)),
    ("impact", |run, io| impact::impact(run.args, run.root, run.options, io)),
    ("explain", |run, io| explain::explain(run.args, run.root, run.options, io)),
    ("where", |run, io| where_(run.args, run.root, run.options, io)),
    ("uncovered", |run, io| uncovered(run.root, run.options, io)),
    ("refresh", |run, io| refresh::refresh(run.args, run.root, io)),
    ("conformance", |run, io| conformance::conformance(run.args, io)),
];

/// Where output goes.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub cwd: PathBuf,
}

impl Io<'_> {
    fn out(&mut self, text: &str) {
        let _ = writeln!(self.out, "{text}");
    }

    fn err(&mut self, text: &str) {
        let _ = writeln!(self.err, "{text}");
    }
}

/// Run the command line; returns the exit code.
pub fn run(argv: &[String], io: &mut Io) -> i32 {
    match dispatch(argv, io) {
        Ok(code) => code,
        Err(Error::User(message)) => {
            io.err(&message);
            1
        }
        Err(internal) => {
            io.err(&format!(
                "docattest: internal error; this is a bug in docattest, please report it.\n{internal}"
            ));
            2
        }
    }
}

fn dispatch(argv: &[String], io: &mut Io) -> Result<i32> {
    let line = parse_command_line(argv)?;
    if line.options.version {
        io.out(&if line.options.json { version_json() } else { version_text() });
        return Ok(0);
    }
    if line.options.help || line.command.as_deref() == Some("help") {
        let topic = if line.command.as_deref() == Some("help") {
            line.args.first().map(String::as_str)
        } else {
            line.command.as_deref()
        };
        io.out(usage::help_for(topic));
        return Ok(0);
    }
    let Some(command) = line.command.as_deref() else {
        io.err(USAGE);
        return Ok(1);
    };
    let Some(&(_, run)) = COMMANDS.iter().find(|(name, _)| *name == command) else {
        return Err(Error::user(format!("unknown command \"{command}\"\n\n{USAGE}")));
    };
    let root = project_root(command, &line.options, &io.cwd);
    run(&Invocation { args: &line.args, root: &root, options: &line.options }, io)
}

fn project_root(command: &str, options: &Options, cwd: &Path) -> PathBuf {
    if let Some(root) = options.root.as_deref() {
        return resolve(cwd, root);
    }
    let creating_config =
        command == "init" && !(options.write_agents || options.hooks || options.git_hook || options.skill);
    if creating_config {
        return resolve(cwd, ".");
    }
    command::find_root(cwd).unwrap_or_else(|| resolve(cwd, "."))
}

/// `--version`: answered before anything is read, and no adapter is started.
fn version_text() -> String {
    format!("docattest {}", env!("CARGO_PKG_VERSION"))
}

/// `--version --json`: what a launcher asks before it trusts a core it did not bring itself.
fn version_json() -> String {
    format!("{{\"version\":\"{}\",\"protocol\":{}}}", env!("CARGO_PKG_VERSION"), crate::adapter::PROTOCOL)
}

/// `root` resolved against `cwd`: absolute, normalised, without a trailing slash. What is
/// absolute is the platform's to say (`C:\p` and `\\server\share` on Windows), so the path
/// is joined and read by its components, never by its slashes.
fn resolve(cwd: &Path, root: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for part in cwd.join(root).components() {
        match part {
            Component::CurDir => {}
            // Above the root is the root, as for a shell's `cd ..`.
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The relative path from one absolute path to another, `..` for each step up, with `/`
/// between its parts, as a project-relative path is written.
fn relative(from: &Path, to: &str) -> String {
    let (from, to) = (resolve(Path::new("/"), &from.to_string_lossy()), resolve(Path::new("/"), to));
    let (a, b): (Vec<Component>, Vec<Component>) = (from.components().collect(), to.components().collect());
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let up = std::iter::repeat_n("..".to_string(), a.len() - common);
    let down = b[common..].iter().map(|part| part.as_os_str().to_string_lossy().into_owned());
    up.chain(down).collect::<Vec<_>>().join("/")
}

/// `DOCATTEST_NETWORK=1` lets examples tagged `network` reach the network.
fn network() -> Network {
    if std::env::var("DOCATTEST_NETWORK").is_ok_and(|v| v == "1") { Network::Allowed } else { Network::Off }
}

fn load(root: &Path) -> Result<State> {
    load_state(DiskFiles::new(root.to_path_buf()), now_ms())
}

/// `--quiet` prints only what needs attention; a check of named pages counts the rest.
fn detail(options: &Options, pages: &[String]) -> Detail {
    match (options.quiet, pages.is_empty()) {
        (true, _) => Detail::Attention,
        (false, true) => Detail::Full,
        (false, false) => Detail::Pages,
    }
}

/// Finish the conversation with the adapter, after a session was opened.
fn finish(state: State) -> Result<()> {
    state.adapter.finish_session()
}

/// `check`, or `check --hook`: an agent's hook.
fn check_command(run: &Invocation, io: &mut Io) -> Result<i32> {
    if run.options.hook {
        if run.options.member.is_some() {
            return Err(Error::user("--member cannot be used with --hook"));
        }
        hook::check_hook(&hook::read_stdin(), run.root, run.options, io)
    } else {
        check::check(run.args, run.root, run.options, io)
    }
}

fn index_of(root: &Path) -> Result<Vec<(String, Vec<Reference>)>> {
    let state = load(root)?;
    let symbols = symbols_only(&state, network())?;
    let index = build_index(&state.attestations, &symbols);
    finish(state)?;
    Ok(index)
}

/// `where [symbol]`: with a symbol, pages/lines/claims that attest it; without one, the
/// whole symbol → references map.
fn where_(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    if args.len() > 1 {
        return Err(Error::user("usage: docattest where [symbol]"));
    }
    let index = index_of(root)?;
    let Some(symbol) = args.first() else {
        print_index(&index, options.json, io);
        return Ok(0);
    };
    let refs: &[Reference] = index.iter().find(|(s, _)| s == symbol).map_or(&[], |(_, refs)| refs);
    if options.json {
        io.out(&pretty(&Value::Array(refs.iter().map(Reference::to_json).collect())));
    } else if refs.is_empty() {
        io.out(&format!("{symbol}: no page attests it"));
    } else {
        for r in refs {
            io.out(&format!("{}:{} {}  {}", r.page, r.line, r.kind.as_str(), r.claim));
        }
    }
    Ok(0)
}

fn print_index(index: &[(String, Vec<Reference>)], json: bool, io: &mut Io) {
    if json {
        let mut map = serde_json::Map::new();
        for (symbol, refs) in index {
            map.insert(symbol.clone(), refs.iter().map(Reference::to_json).collect());
        }
        io.out(&pretty(&Value::Object(map)));
        return;
    }
    for (symbol, refs) in index {
        io.out(symbol);
        for r in refs {
            io.out(&format!("  {}:{} {}  {}", r.page, r.line, r.kind.as_str(), r.claim));
        }
    }
}

/// `uncovered [--surface <name>]`: exports no page attests.
fn uncovered(root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let state = load(root)?;
    let surface = options.surface.as_deref().unwrap_or(DEFAULT_SURFACE);
    let is_internal = state.config.surfaces.iter().any(|(n, e)| n == surface && e.is_internal());
    let names = uncovered_names(&state, network(), surface)?;
    let skips = !state.config.undocumented_globs(surface).is_empty();
    if is_internal {
        state.adapter.finish()?
    } else {
        finish(state)?
    }
    if options.json {
        io.out(&pretty(&serde_json::json!(names)));
    } else {
        print_uncovered_text(surface, is_internal, &names, skips, io);
    }
    Ok(0)
}

fn print_uncovered_text(surface: &str, is_internal: bool, names: &[String], skips: bool, io: &mut Io) {
    if is_internal {
        io.out(&format!("{surface} is an internal surface (not checked for public API coverage)"));
    } else if names.is_empty() && skips {
        io.out("every public symbol is attested or not meant to be documented");
    } else if names.is_empty() {
        io.out("every public symbol is attested");
    } else {
        io.out(&names.join("\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_the_one_every_language_package_pins() {
        let pinned: Value = serde_json::from_str(include_str!("../../protocol/core.json")).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&version_json()).unwrap(), pinned);
        assert_eq!(version_text(), format!("docattest {}", pinned["version"].as_str().unwrap()));
    }

    #[test]
    fn paths_resolve_as_node_does() {
        assert_eq!(resolve(Path::new("/a/b"), "."), PathBuf::from("/a/b"));
        assert_eq!(resolve(Path::new("/a/b"), "../c/"), PathBuf::from("/a/c"));
        assert_eq!(resolve(Path::new("/a"), "/x//y/"), PathBuf::from("/x/y"));
        assert_eq!(relative(Path::new("/a/b"), "/a/b/docs/x.md"), "docs/x.md");
        assert_eq!(relative(Path::new("/a/b"), "/a/c.md"), "../c.md");
        assert_eq!(resolve(Path::new("/a"), "../../.."), PathBuf::from("/"));
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_path_is_absolute_by_its_drive_or_share() {
        let cwd = Path::new(r"C:\work");
        assert_eq!(resolve(cwd, r"D:\p\..\q"), PathBuf::from(r"D:\q"));
        assert_eq!(resolve(cwd, r"\\?\C:\Temp\agent.json"), PathBuf::from(r"\\?\C:\Temp\agent.json"));
        assert_eq!(resolve(cwd, "docs/a.md"), PathBuf::from(r"C:\work\docs\a.md"));
        assert_eq!(relative(Path::new(r"C:\work"), r"C:\work\docs\a.md"), "docs/a.md");
    }
}
