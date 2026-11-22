//! `hooks install --git`: a git pre-commit hook, for a project whose agent has no hooks, or no
//! agent.
//!
//! It runs `docattest check --quiet` on the working tree: the files as they are on disk,
//! which is what every other docattest command reads and what CI will check, and not only
//! what is staged. Reading the index instead would mean checking the project out somewhere
//! else for every commit.

use std::path::{Path, PathBuf};

use crate::api::errors::{Error, Result};
use crate::console::shell::shell_word;
use crate::files::text_file;
use crate::files::text_file::decode;
use crate::git::{Failure, run};

/// The hook's file name in git's hooks directory.
const HOOK: &str = "pre-commit";

/// The pre-commit hook after an install: its file, the line that runs docattest, and
/// whether that line was written now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHook {
    pub file: String,
    pub line: String,
    pub outcome: HookOutcome,
}

/// Whether an install wrote the line, or found it there already.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookOutcome {
    Added,
    AlreadyThere,
}

/// Write the pre-commit hook of the repository `root` is in, where git looks for it
/// (`core.hooksPath` included). A hook that is already there is never changed: one that holds
/// the line this would write is installed, any other is refused, with the line to add to it.
/// `command` runs docattest from the project root.
pub fn install_git_hook(root: &Path, command: &str) -> Result<GitHook> {
    let (prefix, hooks) = git_paths(root)?;
    let path = hooks.join(HOOK);
    let file = path.display().to_string();
    // Git runs the hook at the top of the working tree; the command is the project root's.
    let there = prefix.strip_suffix('/').unwrap_or(&prefix);
    let run = format!("{command} check --quiet");
    let line = if there.is_empty() {
        format!("{run} || exit 1")
    } else {
        format!("(cd {} && {run}) || exit 1", shell_word(there))
    };
    let failed = Error::user;
    if path.exists() {
        if text_file::read_text(&path).map_err(failed)?.split('\n').any(|written| written == line) {
            return Ok(GitHook { file, line, outcome: HookOutcome::AlreadyThere });
        }
        return Err(Error::user(format!(
            "{file} is there already and is left as it is. To check this project before each commit, add this line to it:\n  {line}"
        )));
    }
    let script = format!(
        "#!/bin/sh\n# docattest: the documentation is checked against the code before each commit.\n# Written by `docattest init --git-hook`; delete this file to undo it.\n{line}\n"
    );
    text_file::write_text(&path, &script).map_err(failed)?;
    make_executable(&path).map_err(|e| failed(text_file::failure("make executable", &file, &e)))?;
    Ok(GitHook { file, line, outcome: HookOutcome::Added })
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Where `root` is below the top of its working tree, and git's hooks directory.
fn git_paths(root: &Path) -> Result<(String, PathBuf)> {
    let args = ["rev-parse", "--show-prefix", "--path-format=absolute", "--git-path", "hooks"];
    match run(root, &args, None) {
        Ok(out) => {
            let out = decode(out);
            let mut lines = out.split('\n');
            let prefix = lines.next().unwrap_or_default().to_string();
            // git writes `/` on every platform; rebuilt from its parts, the path is written
            // the platform's way, as the hook's name joined to it will be.
            let hooks = Path::new(lines.next().unwrap_or_default()).components().collect();
            Ok((prefix, hooks))
        }
        Err(Failure::Missing) => Err(Error::user("init --git-hook needs git, and git was not found on PATH")),
        Err(Failure::Failed(_)) => Err(Error::user(format!(
            "init --git-hook needs a git repository, and {} is not inside one",
            root.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Temp, git};
    use std::fs;

    const NPX: &str = "npx --no docattest";

    #[test]
    fn writes_an_executable_hook_where_git_looks_and_never_changes_one_that_is_there() {
        let repo = Temp::new("githook");
        git(&repo.0, &["init", "-q"]);
        let hook = repo.0.join(".git").join("hooks").join("pre-commit");
        let line = "npx --no docattest check --quiet || exit 1";
        let shown = hook.display().to_string();
        assert_eq!(
            install_git_hook(&repo.0, NPX),
            Ok(GitHook { file: shown.clone(), line: line.to_string(), outcome: HookOutcome::Added })
        );
        let script = fs::read_to_string(&hook).unwrap();
        assert!(script.starts_with("#!/bin/sh\n# docattest: "), "{script}");
        assert!(script.ends_with(&format!("\n{line}\n")), "{script}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&hook).unwrap().permissions().mode() & 0o777, 0o755);
        }
        assert_eq!(
            install_git_hook(&repo.0, NPX),
            Ok(GitHook { file: shown.clone(), line: line.to_string(), outcome: HookOutcome::AlreadyThere })
        );
        assert_eq!(fs::read_to_string(&hook).unwrap(), script);
        fs::write(&hook, "#!/bin/sh\nnpm test\n").unwrap();
        let refused = install_git_hook(&repo.0, NPX).unwrap_err().to_string();
        assert_eq!(
            refused,
            format!(
                "docattest: {shown} is there already and is left as it is. To check this project before each commit, add this line to it:\n  {line}"
            )
        );
        assert_eq!(fs::read_to_string(&hook).unwrap(), "#!/bin/sh\nnpm test\n");
    }

    #[test]
    fn follows_core_hooks_path_and_changes_to_a_project_below_the_top() {
        let repo = Temp::new("githook");
        git(&repo.0, &["init", "-q"]);
        git(&repo.0, &["config", "core.hooksPath", ".hooks"]);
        repo.write("packages/my lib/x", "");
        let hook = install_git_hook(&repo.0.join("packages/my lib"), "./.venv/bin/docattest").unwrap();
        assert_eq!(hook.outcome, HookOutcome::Added);
        assert_eq!(hook.file, repo.0.join(".hooks").join("pre-commit").display().to_string());
        assert_eq!(hook.line, "(cd 'packages/my lib' && ./.venv/bin/docattest check --quiet) || exit 1");
        assert!(!repo.0.join(".git/hooks/pre-commit").exists());
    }

    #[test]
    fn needs_a_repository() {
        let plain = Temp::new("githook");
        assert_eq!(
            install_git_hook(&plain.0, NPX).unwrap_err().to_string(),
            format!(
                "docattest: init --git-hook needs a git repository, and {} is not inside one",
                plain.0.display()
            )
        );
    }
}
