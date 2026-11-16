//! Which command starts an adapter: the one its language package's launcher names, or
//! `docattest-adapter-<name>` on the PATH, or one a member's config gives, whose program is
//! found from that config's directory.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::api::errors::{Error, Result};

/// The command for an adapter. The language package's launcher says it, in
/// `DOCATTEST_ADAPTER_<NAME>`: a JSON array of the program and its arguments (PROTOCOL.md
/// §1). For the bare binary in development the variable may also hold a command line, split
/// on spaces, and without it `docattest-adapter-<name>` is looked for on the PATH.
pub(super) fn find(name: &str) -> Result<Vec<String>> {
    let variable = adapter_variable(name);
    if let Ok(command) = std::env::var(&variable) {
        let argv = command_line(&command)
            .map_err(|why| Error::user(format!("{variable} is not a command: {why} (got {command})")))?;
        if !argv.is_empty() {
            return Ok(argv);
        }
    }
    let program = format!("docattest-adapter-{name}");
    // Windows runs a command by the extensions PATHEXT lists: pip installs this one as `.exe`.
    let extensions =
        cfg!(windows).then(|| std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_string()));
    let paths = std::env::var_os("PATH").unwrap_or_default();
    if let Some(found) = on_path(&program, &paths, extensions.as_deref()) {
        return Ok(vec![found.to_string_lossy().into_owned()]);
    }
    Err(Error::user(format!(
        "no {name} adapter: set {variable} to its command, or put {program} on the PATH"
    )))
}

/// What Windows tries when PATHEXT is not set.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// The file the command `program` runs from, in the first directory of `paths` (a PATH)
/// that holds one: named as written, or, where `extensions` are given (PATHEXT's `;`-list),
/// with each of them, as Windows looks for it.
fn on_path(program: &str, paths: &OsStr, extensions: Option<&str>) -> Option<PathBuf> {
    let names: Vec<String> = match extensions {
        None => vec![program.to_string()],
        Some(list) => {
            list.split(';').filter(|ext| !ext.is_empty()).map(|ext| format!("{program}{ext}")).collect()
        }
    };
    std::env::split_paths(paths)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

/// The launcher's variable for the adapter of `name`: `DOCATTEST_ADAPTER_<NAME>`.
pub(super) fn adapter_variable(name: &str) -> String {
    format!("DOCATTEST_ADAPTER_{}", name.to_uppercase())
}

/// A command whose words are read from `dir`: a program named by a relative path is found
/// there, not wherever the core was started (the arguments are the program's to read, and it
/// runs in `dir`).
pub(super) fn from_dir(words: &[String], dir: &Path) -> Vec<String> {
    let mut argv = words.to_vec();
    if let Some(program) = argv.first_mut()
        && program.contains('/')
        && Path::new(program.as_str()).is_relative()
    {
        *program = dir.join(program.as_str()).to_string_lossy().into_owned();
    }
    argv
}

/// A command as a variable holds it: a JSON array of strings when it starts with `[`, so
/// that a path may hold spaces; else words split on spaces.
fn command_line(command: &str) -> std::result::Result<Vec<String>, String> {
    if !command.trim_start().starts_with('[') {
        return Ok(command.split(' ').filter(|s| !s.is_empty()).map(String::from).collect());
    }
    let words: Option<Vec<String>> = serde_json::from_str::<Value>(command)
        .ok()
        .as_ref()
        .and_then(Value::as_array)
        .and_then(|words| words.iter().map(|w| w.as_str().map(str::to_string)).collect());
    words.ok_or_else(|| "a JSON array must hold strings only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_a_json_array_or_words() {
        let words = |text: &str| command_line(text).unwrap();
        assert_eq!(words("node  /a/b.js "), ["node", "/a/b.js"]);
        assert_eq!(words(r#"["/my tools/node", "/a b/main.js"]"#), ["/my tools/node", "/a b/main.js"]);
        assert_eq!(words(" []"), Vec::<String>::new());
        assert_eq!(words(""), Vec::<String>::new());
        assert!(command_line(r#"["node", 1]"#).is_err());
        assert!(command_line("[node").is_err());
    }

    #[test]
    fn an_adapter_is_found_on_the_path_by_its_name_or_with_the_extensions_windows_tries() {
        let (first, second) =
            (crate::git::tests::Temp::new("path-a"), crate::git::tests::Temp::new("path-b"));
        second.write("docattest-adapter-fake.exe", "");
        second.write("docattest-adapter-plain", "");
        let paths = std::env::join_paths([&first.0, &second.0]).unwrap();
        let found = |program: &str, extensions| on_path(program, &paths, extensions);
        let exe = second.0.join("docattest-adapter-fake.exe");
        assert_eq!(found("docattest-adapter-fake", Some(".BAT;;.exe")), Some(exe));
        assert_eq!(found("docattest-adapter-fake", None), None);
        assert_eq!(found("docattest-adapter-plain", None), Some(second.0.join("docattest-adapter-plain")));
        assert_eq!(found("docattest-adapter-none", Some(DEFAULT_PATHEXT)), None);
    }

    #[test]
    fn a_given_program_is_found_from_the_directory_of_the_config() {
        let words = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let dir = Path::new("/repo");
        let at = |program: &str| dir.join(program).to_string_lossy().into_owned();
        assert_eq!(from_dir(&words(&["bin/adapter", "x/y"]), dir), [at("bin/adapter"), "x/y".to_string()]);
        assert_eq!(from_dir(&words(&["./adapter"]), dir), [at("./adapter")]);
        assert_eq!(from_dir(&words(&["python3", "bin/a"]), dir), words(&["python3", "bin/a"]));
        assert_eq!(from_dir(&words(&["/usr/bin/go", "run"]), dir), words(&["/usr/bin/go", "run"]));
    }
}
