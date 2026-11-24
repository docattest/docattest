//! How docattest is run in a project, and where the project is.

use std::path::{Path, PathBuf};

use crate::api::errors::{Error, Result};
use crate::config::CONFIG_FILE;
use crate::text::{is_word, trim};
use serde_json::Value;

/// The variable in which the language package's launcher says how docattest is run in this
/// project: a shell command on one line, to which the core adds arguments (PROTOCOL.md §1).
pub const COMMAND_VARIABLE: &str = "DOCATTEST_COMMAND";

/// The command of the bare binary, which nobody told how it was started.
const BARE_COMMAND: &str = "docattest";

/// In the command, the project root: whoever writes the command down says how it is spelt
/// there.
pub const ROOT: &str = "{root}";

/// The shell command that runs docattest in this project, with `root_as` where it names the
/// project root: an agent's own variable in a hook of that agent, `.` in what is run there.
pub fn docattest_command(root_as: &str) -> Result<String> {
    command_from(std::env::var(COMMAND_VARIABLE).ok().as_deref(), root_as)
}

/// A portable shell command for committed repository files (such as `AGENTS.md`). Falls back
/// to `docattest` instead of baking in a machine-local absolute binary path, returning the
/// replaced path when it does so.
pub fn portable_command(root_as: &str) -> Result<(String, Option<String>)> {
    portable_command_from(std::env::var(COMMAND_VARIABLE).ok().as_deref(), root_as)
}

/// The same, for what the launcher said (or did not).
pub fn portable_command_from(told: Option<&str>, root_as: &str) -> Result<(String, Option<String>)> {
    let command = command_from(told, root_as)?;
    if is_absolute_executable(&command) {
        return Ok((BARE_COMMAND.to_string(), Some(command)));
    }
    Ok((command, None))
}

/// Whether `command` starts with an absolute filesystem path (POSIX, Windows drive, or UNC),
/// optionally quoted for a shell.
fn is_absolute_executable(command: &str) -> bool {
    let first = command.trim_start().trim_start_matches(['\'', '"']);
    if first.starts_with('/') || first.starts_with("\\\\") {
        return true;
    }
    let bytes = first.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/')
}

/// The same, for what the launcher said (or did not).
pub fn command_from(told: Option<&str>, root_as: &str) -> Result<String> {
    let told = trim(told.unwrap_or_default());
    if told.contains(['\n', '\r']) {
        let got = Value::String(told.to_string());
        return Err(Error::user(format!("{COMMAND_VARIABLE} must be one line (got {got})")));
    }
    Ok(if told.is_empty() { BARE_COMMAND } else { told }.replace(ROOT, root_as))
}

/// The project root when `--root` does not name it: the nearest directory at or above
/// `from` that holds a `docattest.config.json`. None when there is none.
pub fn find_root(from: &Path) -> Option<PathBuf> {
    from.ancestors().find(|dir| dir.join(CONFIG_FILE).is_file()).map(Path::to_path_buf)
}

/// Every offset where `word` stands in `text` as a whole word: `/\bword\b/g`.
fn whole_words<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    let boundary = |at: usize| {
        let before = text[..at].chars().next_back().is_some_and(is_word);
        let after = text[at..].chars().next().is_some_and(is_word);
        before != after
    };
    text.match_indices(word).map(|(at, _)| at).filter(move |&at| boundary(at) && boundary(at + word.len()))
}

/// What identifies a docattest command wherever one is written down, whatever else it says:
/// the words `docattest` and `action`, both whole, the action after `docattest` on the same
/// line.
pub fn is_ours(command: &str, action: &str) -> bool {
    const NAME: &str = "docattest";
    let line_ends = |c: char| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    whole_words(command, NAME).any(|at| {
        let from = at + NAME.len();
        whole_words(&command[from..], action)
            // A word boundary at the very start of the rest is one in the whole command too:
            // `docattest` ends in a word character, so an action there would not be whole.
            .any(|found| found > 0 && !command[from..from + found].contains(line_ends))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    #[test]
    fn a_command_is_known_by_its_action() {
        assert!(is_ours("npx --no docattest impact --hook", "impact"));
        assert!(is_ours("\"$SOME_ROOT\"/.venv/bin/docattest impact --hook", "impact"));
        assert!(is_ours("'/my env/bin/python' -m docattest check --hook", "check"));
        assert!(is_ours("docattest check", "check"));
        assert!(is_ours("./node_modules/.bin/docattest --root x impact", "impact"));
        assert!(is_ours("docattest-impact", "impact"));
        assert!(is_ours("mydocattest; docattest impact", "impact"));
        assert!(!is_ours("docattest check", "impact"));
        assert!(!is_ours("impact docattest", "impact"));
        assert!(!is_ours("docattestimpact", "impact"));
        assert!(!is_ours("docattest impacts", "impact"));
        assert!(!is_ours("mydocattest impact", "impact"));
        assert!(!is_ours("docattest \\\n impact", "impact"));
        assert!(is_ours("docattest é impact", "impact"));
    }

    #[test]
    fn the_launchers_command_names_the_root_as_its_reader_spells_it() {
        assert_eq!(command_from(None, "."), Ok("docattest".to_string()));
        assert_eq!(command_from(Some("  "), "."), Ok("docattest".to_string()));
        assert_eq!(command_from(Some(" npx --no docattest\n"), "."), Ok("npx --no docattest".to_string()));
        let told = Some("{root}/.venv/bin/docattest");
        assert_eq!(command_from(told, "."), Ok("./.venv/bin/docattest".to_string()));
        assert_eq!(
            command_from(told, "\"$ROOT_OF_IT\""),
            Ok("\"$ROOT_OF_IT\"/.venv/bin/docattest".to_string())
        );
        assert_eq!(
            command_from(Some("a\nb"), "."),
            Err(Error::User("docattest: DOCATTEST_COMMAND must be one line (got \"a\\nb\")".into()))
        );
    }

    #[test]
    fn portable_command_replaces_machine_local_absolute_paths_with_docattest() {
        assert_eq!(portable_command_from(None, "."), Ok(("docattest".into(), None)));
        assert_eq!(
            portable_command_from(Some("npx --no docattest"), "."),
            Ok(("npx --no docattest".into(), None))
        );
        assert_eq!(
            portable_command_from(Some("{root}/.venv/bin/docattest"), "."),
            Ok(("./.venv/bin/docattest".into(), None))
        );
        assert_eq!(
            portable_command_from(Some("/usr/local/bin/docattest"), "."),
            Ok(("docattest".into(), Some("/usr/local/bin/docattest".into())))
        );
        assert_eq!(
            portable_command_from(Some("'/my tools/bin/docattest'"), "."),
            Ok(("docattest".into(), Some("'/my tools/bin/docattest'".into())))
        );
        assert_eq!(
            portable_command_from(Some(r"C:\tools\docattest.exe"), "."),
            Ok(("docattest".into(), Some(r"C:\tools\docattest.exe".into())))
        );
    }

    #[test]
    fn the_root_is_the_nearest_directory_upwards_that_holds_a_config() {
        let top = Temp::new("root");
        top.write("docattest.config.json", "{}");
        top.write("a/b/c/x.txt", "");
        top.write("a/docattest.config.json", "{}");
        top.write("d/docattest.config.json/not-a-file", "");
        assert_eq!(find_root(&top.0.join("a/b/c")), Some(top.0.join("a")));
        assert_eq!(find_root(&top.0.join("a")), Some(top.0.join("a")));
        assert_eq!(find_root(&top.0.join("d")), Some(top.0.clone()));
        assert_eq!(find_root(&top.0.join("gone/deeper")), Some(top.0.clone()));
        assert_eq!(find_root(Path::new("/nowhere/at/all")), None);
    }
}
