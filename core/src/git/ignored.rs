//! What git ignores in a project: build output, installed packages, generated files. None of
//! it is the project's own, so none of it is a page or a source.

use std::collections::HashSet;
use std::path::Path;

use super::{prefix, run};
use crate::files::text_file::decode;

/// Whether `path` (relative to `root`) is ignored by git. Returns `None` outside a git
/// working tree or when git cannot be run.
pub fn is_ignored(root: &Path, path: &str) -> Option<bool> {
    prefix(root).ok()?;
    Some(run(root, &["check-ignore", "-q", "--", path], None).is_ok())
}

/// The files and directories below `root` that git ignores and does not track, as
/// `.gitignore`, `.git/info/exclude` and the user's excludes say: project-relative, each
/// directory with a trailing `/`. A file git tracks is never among them, whatever a
/// `.gitignore` says. Empty outside a git work tree, when git cannot be run, and when `root`
/// itself is ignored: the root is always the project's own.
pub fn ignored_by_git(root: &Path) -> HashSet<String> {
    let args = ["ls-files", "--others", "--ignored", "--exclude-standard", "--directory", "-z"];
    let Ok(out) = run(root, &args, None) else {
        return HashSet::new();
    };
    let ignored: HashSet<String> =
        decode(out).split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect();
    if ignored.contains(ROOT) { HashSet::new() } else { ignored }
}

/// How git lists the directory it runs in.
const ROOT: &str = "./";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Temp, git, repository};

    fn sorted(set: HashSet<String>) -> Vec<String> {
        let mut paths: Vec<String> = set.into_iter().collect();
        paths.sort();
        paths
    }

    #[test]
    fn ignored_files_and_directories_are_listed_and_tracked_ones_are_not() {
        let repo = repository(&[(".gitignore", "gen/\n*.log\n")]);
        repo.write("docs/kept.log", "tracked");
        git(&repo.0, &["add", "--force", "docs/kept.log"]);
        for file in ["gen/api.md", "docs/a.md", "docs/b.log", "docs/sub/c.log"] {
            repo.write(file, "");
        }
        let listed = sorted(ignored_by_git(&repo.0));
        assert!(["docs/b.log", "docs/sub/", "gen/"].iter().all(|path| listed.iter().any(|p| p == path)));
        assert!(!listed.iter().any(|path| path == "docs/kept.log" || path == "docs/a.md"), "{listed:?}");
        assert!(sorted(ignored_by_git(&repo.0.join("docs"))).contains(&"b.log".to_string()));
    }

    #[test]
    fn outside_a_work_tree_or_inside_an_ignored_directory_nothing_is_ignored() {
        let plain = Temp::new("not-git");
        plain.write("a.log", "");
        assert!(ignored_by_git(&plain.0).is_empty());

        let repo = repository(&[(".gitignore", "scratch/*\n")]);
        repo.write("scratch/project/a.md", "");
        assert!(ignored_by_git(&repo.0.join("scratch/project")).is_empty());
    }

    #[test]
    fn is_ignored_checks_gitignore_inside_a_repo_and_returns_none_outside() {
        let plain = Temp::new("not-git-check-ignore");
        assert_eq!(is_ignored(&plain.0, "local/settings.json"), None);

        let repo = repository(&[(".gitignore", "local/settings.json\n")]);
        assert_eq!(is_ignored(&repo.0, "local/settings.json"), Some(true));
        assert_eq!(is_ignored(&repo.0, "local/other.json"), Some(false));
    }
}
