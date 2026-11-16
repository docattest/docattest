//! What git says changed between a commit and the working tree, for `impact`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{Failure, blobs};
use crate::api::errors::{Error, Result};
use crate::files::text_file::decode;
use crate::text::compare_utf16;

/// The working tree's difference from a commit, below the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitChange {
    /// Project-relative paths that differ, untracked files included. Sorted.
    pub changed: Vec<String>,
    added: HashSet<String>,
    commit: String,
    root: PathBuf,
}

/// Runs git with `args` in `cwd`: its stdout as UTF-8, invalid bytes replaced.
fn run(cwd: &Path, args: &[&str]) -> std::result::Result<String, Failure> {
    super::run(cwd, args, None).map(decode)
}

/// A git command that fails after the repository and the commit were found: unexpected.
fn unexpected(args: &[&str], failure: Failure) -> Error {
    let detail = match failure {
        Failure::Missing => "git was not found".to_string(),
        Failure::Failed(stderr) => stderr,
    };
    Error::Internal { message: format!("git {} failed", args.join(" ")), detail }
}

/// The working tree's difference from `reference`, read with git from the project root. Fails
/// closed: no git, no repository or no such commit is the user's to fix.
pub fn git_change(root: &Path, reference: &str) -> Result<GitChange> {
    if reference.starts_with('-') {
        return Err(Error::user(format!("\"{reference}\" is not a git revision")));
    }
    match run(root, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(_) => {}
        Err(Failure::Missing) => return Err(Error::user("impact needs git, and git was not found on PATH")),
        Err(Failure::Failed(_)) => {
            return Err(Error::user(format!(
                "impact needs a git repository, and {} is not inside one",
                root.display()
            )));
        }
    }
    let Ok(commit) = run(root, &["rev-parse", "--verify", "--quiet", &format!("{reference}^{{commit}}")])
    else {
        return Err(Error::user(format!("\"{reference}\" is not a commit in this repository")));
    };
    let commit = commit.trim().to_string();
    let mut added = HashSet::new();
    let mut changed = HashSet::new();
    let diff_args = ["diff", "--name-status", "--no-renames", "-z", "--relative", &commit, "--"];
    let diff = run(root, &diff_args).map_err(|f| unexpected(&diff_args, f))?;
    let fields: Vec<&str> = diff.split('\0').collect();
    for [status, file] in fields.as_chunks::<2>().0 {
        changed.insert(file.to_string());
        if status.starts_with('A') {
            added.insert(file.to_string());
        }
    }
    let others_args = ["ls-files", "--others", "--exclude-standard", "-z"];
    let others = run(root, &others_args).map_err(|f| unexpected(&others_args, f))?;
    for file in others.split('\0').filter(|f| !f.is_empty()) {
        changed.insert(file.to_string());
        added.insert(file.to_string());
    }
    let mut changed: Vec<String> = changed.into_iter().collect();
    changed.sort_by(|a, b| compare_utf16(a, b));
    Ok(GitChange { changed, added, commit, root: root.to_path_buf() })
}

impl GitChange {
    /// The same difference narrowed to the one project file `file`: nothing when the
    /// difference does not touch it.
    pub fn only(mut self, file: &str) -> GitChange {
        self.changed.retain(|changed| changed == file);
        self
    }

    fn existed(&self, file: &str) -> bool {
        !self.added.contains(file) && self.changed.binary_search_by(|c| compare_utf16(c, file)).is_ok()
    }

    /// A changed file's contents at the commit; None when it did not exist there (or was
    /// not a file there: a submodule has no contents).
    pub fn read(&self, file: &str) -> Option<String> {
        if !self.existed(file) {
            return None;
        }
        run(&self.root, &["show", &format!("{}:./{file}", self.commit)]).ok()
    }

    /// The contents at the commit of every changed path, in the order of `changed`: what
    /// the `affected` request carries. One `git cat-file --batch` reads them all; a path it
    /// cannot answer plainly (not a plain blob there) is read on its own.
    pub fn base_contents(&self) -> Vec<(String, Option<String>)> {
        let wanted: Vec<&str> = self
            .changed
            .iter()
            .map(String::as_str)
            .filter(|f| self.existed(f) && !f.contains('\n'))
            .collect();
        let batch = if wanted.len() > 2 { self.batch(&wanted) } else { Vec::new() };
        let mut batch = wanted.iter().copied().zip(batch).collect::<std::collections::HashMap<_, _>>();
        self.changed
            .iter()
            .map(|file| {
                let text = match batch.remove(file.as_str()) {
                    Some(Some(text)) => Some(text),
                    _ => self.read(file),
                };
                (file.clone(), text)
            })
            .collect()
    }

    /// The blobs at `files`, in order; None where the answer was not a blob. Shorter than
    /// `files` when git stopped early, and empty when it failed.
    fn batch(&self, files: &[&str]) -> Vec<Option<String>> {
        let objects: Vec<String> = files.iter().map(|file| format!("{}:./{file}", self.commit)).collect();
        blobs(&self.root, &objects).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Temp, git, repository};
    use std::fs;

    fn message(result: Result<GitChange>) -> String {
        match result {
            Err(Error::User(m)) => m,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn lists_modified_deleted_and_untracked_files_and_reads_their_base_contents() {
        let repo = repository(&[("a.ts", "a1"), ("b.ts", "b1"), ("c.ts", "c1")]);
        repo.write("a.ts", "a2");
        fs::remove_file(repo.0.join("b.ts")).unwrap();
        repo.write("new.ts", "n");
        let change = git_change(&repo.0, "HEAD").unwrap();
        assert_eq!(change.changed, vec!["a.ts", "b.ts", "new.ts"]);
        assert_eq!(change.read("a.ts").as_deref(), Some("a1"));
        assert_eq!(change.read("b.ts").as_deref(), Some("b1"));
        assert_eq!(change.read("new.ts"), None);
        assert_eq!(change.read("c.ts"), None);
        assert_eq!(
            change.base_contents(),
            vec![
                ("a.ts".to_string(), Some("a1".to_string())),
                ("b.ts".to_string(), Some("b1".to_string())),
                ("new.ts".to_string(), None)
            ]
        );
        let narrowed = git_change(&repo.0, "HEAD").unwrap().only("b.ts");
        assert_eq!(narrowed.changed, vec!["b.ts"]);
        assert_eq!(narrowed.base_contents(), vec![("b.ts".to_string(), Some("b1".to_string()))]);
        assert!(git_change(&repo.0, "HEAD").unwrap().only("c.ts").changed.is_empty());
    }

    #[test]
    fn works_from_a_project_in_a_subdirectory_with_paths_relative_to_it() {
        let repo = repository(&[("sub/a.ts", "a1"), ("other.ts", "o1")]);
        repo.write("sub/a.ts", "a2");
        repo.write("other.ts", "o2");
        let change = git_change(&repo.0.join("sub"), "HEAD").unwrap();
        assert_eq!(change.changed, vec!["a.ts"]);
        assert_eq!(change.read("a.ts").as_deref(), Some("a1"));
    }

    #[test]
    fn fails_closed_outside_a_repository_and_on_a_revision_that_does_not_exist() {
        let plain = Temp::new("nogit");
        assert!(message(git_change(&plain.0, "HEAD")).contains("needs a git repository"));
        let repo = repository(&[("a.ts", "a")]);
        assert_eq!(
            message(git_change(&repo.0, "no-such-branch")),
            "docattest: \"no-such-branch\" is not a commit in this repository"
        );
        assert_eq!(
            message(git_change(&repo.0, "--output=x")),
            "docattest: \"--output=x\" is not a git revision"
        );
    }

    #[test]
    fn a_repository_without_commits_has_no_head() {
        let temp = Temp::new("empty");
        git(&temp.0, &["init", "-q", "-b", "main"]);
        assert_eq!(
            message(git_change(&temp.0, "HEAD")),
            "docattest: \"HEAD\" is not a commit in this repository"
        );
    }

    #[test]
    fn many_files_are_read_in_one_batch_from_a_subdirectory_too() {
        let names: Vec<String> = (0..12).map(|k| format!("p/f {k}é.ts")).collect();
        let mut files: Vec<(&str, &str)> =
            names.iter().map(|n| (n.as_str(), "old\n\u{0}\u{1F600}")).collect();
        files.push(("p/empty.ts", ""));
        files.push(("p/gone.ts", "gone"));
        let repo = repository(&files);
        for name in &names {
            repo.write(name, "new");
        }
        repo.write("p/empty.ts", "x");
        repo.write("p/new file.ts", "n");
        fs::remove_file(repo.0.join("p/gone.ts")).unwrap();
        let change = git_change(&repo.0.join("p"), "main").unwrap();
        let contents = change.base_contents();
        assert_eq!(contents.len(), 15);
        for (file, text) in &contents {
            let single = change.read(file);
            assert_eq!(*text, single, "{file}");
            let expected = match file.as_str() {
                "empty.ts" => Some(""),
                "gone.ts" => Some("gone"),
                "new file.ts" => None,
                _ => Some("old\n\u{0}\u{1F600}"),
            };
            assert_eq!(text.as_deref(), expected, "{file}");
        }
    }
}
