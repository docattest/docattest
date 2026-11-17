//! `impact --hook` and `check --hook`.
//!
//! Hook mode: a coding agent runs the command on one of its events. The event arrives as
//! JSON on stdin; a hook that has something to say prints one JSON object on stdout and exits
//! 0, and one that has nothing to say prints nothing. Where the event holds what, and what
//! shape the answer takes, is the agent's descriptor's to say (`docs/agents.md`).

use std::io::Read;
use std::path::{Path, PathBuf};

use super::args::Options;
use super::check::check_projects;
use super::command::find_root;
use super::{Io, relative, resolve};
use crate::adapter::Start;
use crate::agents::{Binding, load_agent, reply_with, value_at};
use crate::api::errors::{Error, Result};
use crate::files::source::DiskFiles;
use crate::impact::brief::brief_impact;
use crate::impact::commands::builds_any;
use crate::impact::{Impact, Scope};
use crate::project::state::{Started, start};
use crate::report::Detail;
use serde_json::{Map, Value};

/// Standard input, read whole and decoded with replacement; empty when it cannot be read. A
/// hook never fails on what the agent sends: that would interrupt the agent over its own
/// event, so input that cannot be read or parsed counts as no event at all.
pub(super) fn read_stdin() -> String {
    let mut bytes = Vec::new();
    if std::io::stdin().lock().read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The event: the JSON object on stdin, or an empty one for anything else.
fn read_event(stdin: &str) -> Map<String, Value> {
    serde_json::from_str(stdin).unwrap_or_default()
}

/// The string at a path in the event, when there is one and it is not empty.
fn string_at<'a>(event: &'a Map<String, Value>, path: &[String]) -> Option<&'a str> {
    match value_at(event, path) {
        Some(Value::String(value)) if !value.is_empty() => Some(value),
        _ => None,
    }
}

/// The project-relative form of an absolute path, or None when it lies outside.
fn relative_to(root: &Path, file: &Path) -> Option<String> {
    let rel = relative(root, &file.to_string_lossy());
    (!rel.starts_with("..")).then_some(rel)
}

/// The agent's event for an action; an agent that has none cannot run it as a hook.
fn bound(action: &str, options: &Options, io: &Io) -> Result<Binding> {
    let agent = load_agent(options.agent.as_deref(), |given| resolve(&io.cwd, given))?.agent;
    match agent.binding(action) {
        Some(binding) => Ok(binding.clone()),
        None => Err(Error::user(format!(
            "agent \"{}\" has no hook for {action}: its descriptor binds none",
            agent.name
        ))),
    }
}

/// Where an edited file is, as the core decides it alone, before any adapter is started.
pub(super) enum Edited {
    /// The file `rel` of the project at `root`.
    Inside { root: PathBuf, rel: String },
    /// A file outside the project at `root`.
    Outside { root: PathBuf },
}

/// Where the edited `file` (absolute) is. The project is the one `--root` names, else the
/// one the file is in, else the one the working directory is in (`root`, either way, when
/// the file names none).
pub(super) fn locate(file: &Path, root: &Path, options: &Options) -> Edited {
    let found = if options.root.is_some() { None } else { file.parent().and_then(find_root) };
    let root = found.unwrap_or_else(|| root.to_path_buf());
    match relative_to(&root, file) {
        Some(rel) => Edited::Inside { root, rel },
        None => Edited::Outside { root },
    }
}

/// What the change in the working tree affects, asked after the file `rel` of the project at
/// `root` was edited: nothing unless it is a source file, or one a command is built from, and
/// its change affects an export or a command.
pub(super) fn impact_of_edit(root: &Path, rel: &str, options: &Options) -> Result<Option<Impact>> {
    // What the file is to the language is the language's to say, and its greeting says it:
    // the adapter is started cold, since the greeting may be all that is asked of it.
    after_edit(start(DiskFiles::new(root.to_path_buf()), Start::Cold)?, rel, options)
}

/// The rest, for an edit to the project file `rel`: the adapter says in its greeting which
/// names are never the project's own and which files are source files, the config which
/// files its commands are built from, and the impact, when there is one to work out, goes on
/// with the adapter that was started for that.
fn after_edit(started: Started, rel: &str, options: &Options) -> Result<Option<Impact>> {
    let greeted = started.greet()?;
    let adapter = &greeted.adapter;
    let edited = [rel.to_string()];
    let code = adapter.source_files.is_match(rel) || builds_any(&greeted.config.commands, &edited)?;
    if adapter.ignored.on_disk(greeted.files.root(), rel) || !code {
        greeted.adapter.finish()?;
        return Ok(None);
    }
    let found = super::impact::impact_on(greeted, options, &Scope::File(rel.to_string()))?;
    Ok((!found.is_empty()).then_some(found))
}

/// `impact --hook`, after the agent edited a file. Silent unless the edited file is a source
/// file of the project and a page names what its change affects; then a brief of the impact
/// (see [`brief_impact`]) goes into the agent's context next to the edit, small because it
/// comes after every edit. An agent edits files anywhere, so one outside the
/// project is not a mistake, only none of the project's business.
pub(super) fn impact_hook(stdin: &str, root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let binding = bound("impact", options, io)?;
    let event = read_event(stdin);
    let Some(edited) = string_at(&event, binding.file.as_deref().unwrap_or_default()) else {
        return Ok(0);
    };
    let Edited::Inside { root, rel } = locate(&resolve(&io.cwd, edited), root, options) else {
        return Ok(0);
    };
    if let Some(brief) = impact_of_edit(&root, &rel, options)?.and_then(|found| brief_impact(&found, &rel)) {
        io.out(&reply_with(&binding.reply, brief));
    }
    Ok(0)
}

/// Whether the agent says it is already continuing because of a hook: `true`, or a count
/// above 0.
fn is_continuing(flag: Option<&Value>) -> bool {
    match flag {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(count)) => count.as_f64().is_some_and(|count| count > 0.0),
        _ => false,
    }
}

/// `check --hook`, before the agent stops. A failing check keeps the agent working, with the
/// failures as the reason; once only, so a failure it cannot fix never loops.
pub(super) fn check_hook(stdin: &str, root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let binding = bound("check", options, io)?;
    if let Some(path) = &binding.continuing
        && is_continuing(value_at(&read_event(stdin), path))
    {
        return Ok(0);
    }
    let checked = check_projects(&[], root, None)?;
    if checked.exit_code() == 0 {
        return Ok(0);
    }
    let reason = format!(
        "docattest check failed: these pages no longer match the code.\n{}",
        checked.format(Detail::Attention)
    );
    io.out(&reply_with(&binding.reply, reason));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{Adapter, Launch, Starting};
    use crate::cli::hooks::tests::FLAT;
    use crate::git::tests::Temp;
    use crate::impact::text::format_impact;
    use std::path::PathBuf;

    fn at(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn the_event_is_an_object_or_nothing() {
        assert!(read_event("").is_empty());
        assert!(read_event("[1]").is_empty());
        assert!(read_event("{ nope").is_empty());
        assert!(read_event("\u{FEFF}{}").is_empty());
        let event = read_event(r#"{"in":{"path":"/a/b.ts","empty":"","n":1},"x":"y"}"#);
        assert_eq!(string_at(&event, &at(&["in", "path"])), Some("/a/b.ts"));
        assert_eq!(string_at(&event, &at(&["in", "empty"])), None);
        assert_eq!(string_at(&event, &at(&["in", "n"])), None);
        assert_eq!(string_at(&event, &at(&["x", "path"])), None);
        assert_eq!(string_at(&event, &at(&["gone", "path"])), None);
        assert_eq!(string_at(&event, &at(&["x"])), Some("y"));
        assert_eq!(string_at(&event, &[]), None);
    }

    #[test]
    fn an_agent_is_continuing_when_it_says_true_or_counts_above_nothing() {
        let event = read_event(r#"{"a":true,"b":false,"c":"true","d":0,"e":1,"f":null,"g":-1,"h":{"i":2}}"#);
        let continuing = |keys: &[&str]| is_continuing(value_at(&event, &at(keys)));
        assert!(continuing(&["a"]) && continuing(&["e"]) && continuing(&["h", "i"]));
        for not in ["b", "c", "d", "f", "g", "h", "gone"] {
            assert!(!continuing(&[not]), "{not}");
        }
    }

    #[test]
    fn paths_outside_the_project_are_not_its_files() {
        let root = PathBuf::from("/work/p");
        assert_eq!(relative_to(&root, Path::new("/work/p/src/a.ts")).as_deref(), Some("src/a.ts"));
        assert_eq!(relative_to(&root, Path::new("/work/p")).as_deref(), Some(""));
        assert_eq!(relative_to(&root, Path::new("/work/q/a.ts")), None);
        assert_eq!(relative_to(&root, Path::new("/work/p/..a.ts")), None);
    }

    /// Run a hook of the agent nobody ships (`FLAT`), on a project whose root is named as
    /// `--root` names it, so the edited file does not choose it.
    fn run_in(root: &Path, stdin: &str, hook: &str) -> (i32, String, String) {
        let descriptor = Temp::new("agent");
        descriptor.write("flat.json", FLAT);
        let options = Options {
            root: Some(root.to_string_lossy().into_owned()),
            agent: Some(descriptor.0.join("flat.json").to_string_lossy().into_owned()),
            ..Options::default()
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io { out: &mut out, err: &mut err, cwd: PathBuf::from("/work") };
        let result = match hook {
            "check" => check_hook(stdin, root, &options, &mut io),
            _ => impact_hook(stdin, root, &options, &mut io),
        };
        let code = match result {
            Ok(code) => code,
            Err(e) => {
                io.err(&e.to_string());
                1
            }
        };
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    fn run(stdin: &str, hook: &str) -> (i32, String, String) {
        run_in(Path::new("/nowhere/at/all"), stdin, hook)
    }

    #[test]
    fn lets_the_agent_stop_after_one_reminder_so_a_failure_it_cannot_fix_never_loops() {
        let quiet = (0, String::new(), String::new());
        assert_eq!(run(r#"{"retries":1}"#, "check"), quiet);
        assert_eq!(run(r#"{"retries":true}"#, "check"), quiet);
        for first in [r#"{"retries":0}"#, r#"{"retries":"1"}"#, "{}", ""] {
            let (code, out, err) = run(first, "check");
            assert_eq!((code, out.as_str()), (1, ""), "{first}");
            assert_eq!(err, "docattest: no docattest.config.json in /nowhere/at/all\n");
        }
    }

    #[test]
    fn an_agent_without_the_event_cannot_run_the_hook() {
        let descriptor = Temp::new("agent");
        let only_check = FLAT.replace(r#""impact": { "event": "edited", "matcher": "*.src", "file": ["path"], "reply": { "text": ["say", "to", "agent"] } },"#, "");
        assert_ne!(only_check, FLAT);
        descriptor.write("flat.json", &only_check);
        let agent = Some(descriptor.0.join("flat.json").to_string_lossy().into_owned());
        let options = Options { agent, ..Options::default() };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io { out: &mut out, err: &mut err, cwd: PathBuf::from("/work") };
        assert_eq!(
            impact_hook("{}", Path::new("/nowhere"), &options, &mut io).unwrap_err().to_string(),
            "docattest: agent \"flat\" has no hook for impact: its descriptor binds none"
        );
    }

    const CONFIG: &str =
        r#"{"pages":["docs/**/*.md"],"surfaces":{"default":"./src/index.ts"},"adapter":"typescript"}"#;

    /// An adapter for a language the core has never heard of: it greets, naming `*.fake`
    /// files (but not `*.not.fake`) as its source files, and answers nothing else.
    fn fake_adapter(project: &Temp) -> Starting {
        let hello = concat!(
            r#"{"id":1,"result":{"protocol":1,"adapter":{"name":"fake","version":"0"},"#,
            r#""blockLanguages":["fake"],"capabilities":[],"ignoredNames":["deps"],"#,
            r#""sourceFiles":{"include":["**/*.fake"],"exclude":["**/*.not.fake"]}}}"#
        );
        project.write("adapter.sh", &format!("read hello\nprintf '%s\\n' '{hello}'\ncat >/dev/null\n"));
        let script = project.0.join("adapter.sh").to_string_lossy().into_owned();
        Adapter::start_command(&["sh".to_string(), script], "fake", Start::Cold).unwrap()
    }

    fn after(project: &Temp, file: &str) -> (i32, String, String) {
        let files = DiskFiles::new(project.0.clone());
        let config = crate::config::load_config(&files).unwrap();
        let started = Started { files, config, adapter: fake_adapter(project), launch: Launch::Found };
        match after_edit(started, file, &Options::default()) {
            Ok(None) => (0, String::new(), String::new()),
            Ok(Some(found)) => (0, format_impact(&found), String::new()),
            Err(e) => (1, String::new(), format!("{e}\n")),
        }
    }

    #[test]
    fn the_adapter_says_which_edited_files_are_source_files() {
        let project = crate::git::tests::repository(&[
            ("docattest.config.json", CONFIG),
            ("src/index.ts", "export const n = 1;"),
            ("docs/n.md", "Use `n`."),
        ]);
        // Not source files of this adapter's language, TypeScript included: silence.
        let quiet = (0, String::new(), String::new());
        // Nor is anything under a name it ignores, and `node_modules` is not one of those.
        for file in [
            "docs/n.md",
            "src/index.ts",
            "src/a.not.fake",
            "package.json",
            "fake",
            "src/a.FAKE",
            "",
            "deps/x/a.fake",
            "src/deps/a.fake",
        ] {
            assert_eq!(after(&project, file), quiet, "{file}");
        }
        // Its source files, hidden ones too: the impact is worked out, as far as this
        // adapter can take it.
        for file in
            ["src/a.fake", ".fake", "src/.cache/a.not.fake.fake", "node_modules/x/a.fake", "deps.fake"]
        {
            let (code, out, err) = after(&project, file);
            assert_eq!((code, out.as_str()), (1, ""), "{file}");
            assert_eq!(
                err, "docattest: the typescript adapter cannot tell what a change affects\n",
                "{file}"
            );
        }
    }

    #[test]
    fn stays_silent_after_an_edit_to_anything_outside_the_project() {
        let quiet = (0, String::new(), String::new());
        for event in ["", "{}", r#"{"path":7}"#, r#"{"path":""}"#, r#"{"path":"/elsewhere/a.ts"}"#] {
            assert_eq!(run(event, "impact"), quiet, "{event}");
        }
        // Inside the project the config is read first, whatever the file: which names are
        // dependencies is the adapter's to say, and the config names the adapter.
        for file in ["docs/a.md", "node_modules/x/a.ts", ""] {
            let event = format!(r#"{{"path":"/nowhere/at/all/{file}"}}"#);
            let (code, _, err) = run(&event, "impact");
            assert_eq!((code, err.as_str()), (1, "docattest: no docattest.config.json in /nowhere/at/all\n"));
        }
    }

    #[test]
    fn the_edited_file_names_its_project_when_no_root_is_given() {
        let top = Temp::new("edited");
        top.write("inner/src/a.txt", "");
        top.write("inner/docattest.config.json", "{ not a config");
        let file = top.0.join("inner/src/a.txt");
        let inside = |edited: Edited| match edited {
            Edited::Inside { root, rel } => (root, rel),
            Edited::Outside { root } => panic!("outside {}", root.display()),
        };
        // Neither the working directory nor anything above it holds a config: the file's
        // own project is found, and its config is what is read.
        let (root, rel) = inside(locate(&file, &top.0, &Options::default()));
        assert_eq!((root.as_path(), rel.as_str()), (top.0.join("inner").as_path(), "src/a.txt"));
        let message = impact_of_edit(&root, &rel, &Options::default()).unwrap_err().to_string();
        assert!(message.starts_with("docattest: docattest.config.json is not valid JSON"), "{message}");
        // A root that is named stands: the file is in it, and it has no config.
        let named = Options { root: Some(top.0.to_string_lossy().into_owned()), ..Options::default() };
        let (root, rel) = inside(locate(&file, &top.0, &named));
        assert_eq!((root.as_path(), rel.as_str()), (top.0.as_path(), "inner/src/a.txt"));
        assert!(matches!(
            locate(Path::new("/elsewhere/a.txt"), &top.0, &Options::default()),
            Edited::Outside { .. }
        ));
    }
}
