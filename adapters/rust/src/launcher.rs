//! What the `docattest` command of this package hands over to the core (PROTOCOL.md,
//! section 1, "Launchers"): how to start the Rust adapter, and what command runs
//! docattest here.
//!
//! The core is written in Rust, so this package does not carry it as a separate program:
//! the command is built with the core inside it, and runs it. `DOCATTEST_CORE` still
//! names another core to run in its place, which is asked what protocol it speaks.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The variable that names another core to run.
pub const CORE_VARIABLE: &str = "DOCATTEST_CORE";
/// The variable that says how to start this package's adapter.
pub const ADAPTER_VARIABLE: &str = "DOCATTEST_ADAPTER_RUST";
/// The variable that says what command runs docattest.
pub const COMMAND_VARIABLE: &str = "DOCATTEST_COMMAND";

/// The commands that write the docattest command down; only for them is it worked out.
const WRITERS: [&str; 2] = ["hooks", "agents"];

fn executable(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// A program by name in the directories of a `PATH`.
fn on_path(name: &str, path: &str) -> Option<PathBuf> {
    std::env::split_paths(path).map(|dir| dir.join(executable(name))).find(|candidate| candidate.is_file())
}

/// How to start this package's adapter: the program installed beside the command, else
/// the one on the `PATH`. `None` when there is neither; the core then says where it
/// looked.
fn adapter_command(command: &Path, path: &str) -> Option<Vec<String>> {
    let beside = command
        .parent()
        .map(|dir| dir.join(executable("docattest-adapter-rust")))
        .filter(|file| file.is_file());
    let found = beside.or_else(|| on_path("docattest-adapter-rust", path))?;
    Some(vec![found.to_string_lossy().into_owned()])
}

/// A word as a POSIX shell reads it back.
fn quote(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c);
    if !word.is_empty() && word.chars().all(plain) {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// A shell command that runs docattest: `docattest` when that is what a shell finds on
/// the `PATH`, else this command by its full path.
fn docattest_command(command: &Path, path: &str) -> String {
    let same = |found: PathBuf| match (found.canonicalize(), command.canonicalize()) {
        (Ok(found), Ok(command)) => found == command,
        _ => false,
    };
    if on_path("docattest", path).is_some_and(same) {
        return "docattest".to_string();
    }
    quote(&command.to_string_lossy())
}

/// What the hand-over adds to the environment: each variable that is not already set,
/// so that a developer's own adapter and a wrapper's own command stand.
pub fn hand_over(
    command: &Path,
    args: &[String],
    get: &dyn Fn(&str) -> Option<String>,
) -> Vec<(&'static str, String)> {
    let path = get("PATH").unwrap_or_default();
    let unset = |name: &str| get(name).is_none_or(|value| value.is_empty());
    let mut added = Vec::new();
    if let Some(adapter) = adapter_command(command, &path).filter(|_| unset(ADAPTER_VARIABLE)) {
        added.push((ADAPTER_VARIABLE, serde_json::json!(adapter).to_string()));
    }
    if unset(COMMAND_VARIABLE) && args.iter().any(|arg| WRITERS.contains(&arg.as_str())) {
        added.push((COMMAND_VARIABLE, docattest_command(command, &path)));
    }
    added
}

/// Checks a core someone named: it must answer `--version --json`, and speak this
/// package's protocol.
pub fn checked(core: &str, answer: &str) -> Result<(), String> {
    let told: serde_json::Value = serde_json::from_str(answer).unwrap_or_default();
    let (Some(version), Some(protocol)) = (told["version"].as_str(), told["protocol"].as_f64()) else {
        return Err(format!(
            "docattest: {CORE_VARIABLE} ({core}) is not a docattest core: it does not answer --version --json"
        ));
    };
    let ours = docattest::adapter::PROTOCOL;
    if protocol != ours as f64 {
        return Err(format!(
            "docattest: {CORE_VARIABLE} ({core}) is core {version}, which speaks adapter protocol {protocol}; this package speaks protocol {ours}"
        ));
    }
    Ok(())
}

/// Asks a core what it is.
pub fn ask(core: &str) -> String {
    let asked = Command::new(core).args(["--version", "--json"]).output();
    asked
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_core_someone_names_is_asked_what_it_speaks() {
        assert_eq!(checked("/a/core", r#"{"version":"0.1.0","protocol":1}"#), Ok(()));
        assert!(
            checked("/a/core", r#"{"version":"9.0.0","protocol":2}"#)
                .unwrap_err()
                .contains("speaks adapter protocol 2; this package speaks protocol 1")
        );
        assert!(checked("/a/core", "hello").unwrap_err().contains("is not a docattest core"));
    }

    #[test]
    fn words_are_quoted_for_a_shell() {
        assert_eq!(quote("/usr/bin/docattest"), "/usr/bin/docattest");
        assert_eq!(quote("/my tools/it's"), r"'/my tools/it'\''s'");
    }

    #[test]
    fn the_hand_over_adds_only_what_is_not_set() {
        let dir = std::env::temp_dir().join(format!("docattest-launcher-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (command, adapter) = (dir.join("docattest"), dir.join(executable("docattest-adapter-rust")));
        std::fs::write(&adapter, "").unwrap();
        let none = |_: &str| None;
        let added = hand_over(&command, &["check".to_string()], &none);
        assert_eq!(added, vec![(ADAPTER_VARIABLE, format!("[{:?}]", adapter.to_string_lossy()))]);
        let writes = hand_over(&command, &["hooks".to_string(), "install".to_string()], &none);
        assert_eq!(writes[1], (COMMAND_VARIABLE, command.to_string_lossy().into_owned()));
        let set = |name: &str| (name != "PATH").then(|| "mine".to_string());
        assert!(hand_over(&command, &["hooks".to_string()], &set).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
