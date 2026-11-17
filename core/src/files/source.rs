//! Project-relative paths and the project on disk.

use super::project_files::ProjectFiles;
use super::text_file;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::ignored::Ignored;

/// A project-relative path in canonical form: forward slashes, no `./`, no doubled `/`.
pub fn normalise(file: &str) -> String {
    let slashed = file.replace('\\', "/");
    let stripped = slashed.strip_prefix("./").unwrap_or(&slashed);
    let mut out = String::with_capacity(stripped.len());
    for c in stripped.chars() {
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    out
}

/// A POSIX path normalised: `.` and empty segments dropped, each `..` resolved (see
/// [`resolve_segments`]), a trailing slash kept; `.` for an empty relative path.
pub fn posix_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut out = resolve_segments(path, absolute).join("/");
    if out.is_empty() && !absolute {
        out = ".".to_string();
    }
    if !out.is_empty() && trailing {
        out.push('/');
    }
    if absolute {
        out.insert(0, '/');
    }
    out
}

/// The path's segments without `.` and empty ones, each `..` taking away the segment
/// before it (above the root of an absolute path, nothing).
fn resolve_segments(path: &str, absolute: bool) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    parts
}

/// A project in a directory on disk.
#[derive(Debug, Clone)]
pub struct DiskFiles {
    root: PathBuf,
    /// What is never the project's own, whatever it holds: the adapter's to say.
    ignored: Ignored,
    /// What git ignores: project-relative paths, a directory's with a trailing `/`.
    ignored_by_git: Arc<HashSet<String>>,
}

impl DiskFiles {
    pub fn new(root: PathBuf) -> DiskFiles {
        DiskFiles { root, ignored: Ignored::default(), ignored_by_git: Arc::default() }
    }

    /// Leave out of [`DiskFiles::list`] every file and directory the language ignores.
    pub fn ignore(&mut self, ignored: &Ignored) {
        self.ignored = ignored.clone();
    }

    /// Leave out of [`DiskFiles::list`] the files and directories git ignores, as
    /// [`crate::git::ignored::ignored_by_git`] lists them.
    pub fn ignore_what_git_ignores(&mut self, ignored_by_git: HashSet<String>) {
        self.ignored_by_git = Arc::new(ignored_by_git);
    }

    /// The absolute root, as text.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, file: &str) -> PathBuf {
        self.root.join(file)
    }

    /// Files directly inside a directory, sorted; hidden directories included.
    pub fn list_directory(&self, dir: &str) -> Vec<String> {
        let dir = dir.strip_suffix('/').unwrap_or(dir);
        let mut out: Vec<String> = entries(&self.root.join(dir))
            .into_iter()
            .filter(|e| e.is_file)
            .map(|e| format!("{dir}/{}", e.name))
            .collect();
        out.sort_by(|a, b| crate::text::compare_utf16(a, b));
        out
    }
}

impl ProjectFiles for DiskFiles {
    /// The text of a project file, decoded as UTF-8 with replacement.
    fn read(&self, file: &str) -> Result<String, String> {
        fs::read(self.path(file)).map(text_file::decode).map_err(|e| text_file::failure("read", file, &e))
    }

    fn exists(&self, file: &str) -> bool {
        fs::metadata(self.path(file)).is_ok_and(|m| m.is_file())
    }

    /// Every project file, sorted. Tool state (hidden directories) is never a page or a
    /// source, and neither is anything the language or git ignores.
    fn list(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk("", &mut out);
        out
    }
}

struct Entry {
    name: String,
    is_dir: bool,
    is_file: bool,
}

// A directory's entries, by their own type: a symlink is neither a file nor a directory, so
// the walk neither lists nor follows one.
fn entries(dir: &Path) -> Vec<Entry> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter_map(|e| {
            let kind = e.file_type().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: kind.is_dir(),
                is_file: kind.is_file(),
            })
        })
        .collect()
}

impl DiskFiles {
    fn walk(&self, rel: &str, out: &mut Vec<String>) {
        let dir = if rel.is_empty() { self.root.clone() } else { self.root.join(rel) };
        let mut here: Vec<String> = Vec::new();
        let mut found = entries(&dir);
        // A marker says the directory that holds it is not the project's own; the root always is.
        if !rel.is_empty() && found.iter().any(|entry| entry.is_file && self.ignored.marker(&entry.name)) {
            return;
        }
        found.sort_by(|a, b| crate::text::compare_utf16(&a.name, &b.name));
        for entry in &found {
            let child = if rel.is_empty() { entry.name.clone() } else { format!("{rel}/{}", entry.name) };
            if self.leaves_out(entry, &child) {
                continue;
            }
            if entry.is_dir {
                self.walk(&child, &mut here);
            } else if entry.is_file {
                here.push(child);
            }
        }
        // Each directory's list is sorted as a whole, nested paths included.
        here.sort_by(|a, b| crate::text::compare_utf16(a, b));
        out.extend(here);
    }

    /// An ignored name, a hidden directory, or what git ignores; `child` is the entry's path
    /// from the project's root, as git lists it (a directory's with a trailing `/`).
    fn leaves_out(&self, entry: &Entry, child: &str) -> bool {
        let hidden_dir = entry.is_dir && entry.name.starts_with('.');
        let by_git = if entry.is_dir {
            self.ignored_by_git.contains(&format!("{child}/"))
        } else {
            self.ignored_by_git.contains(child)
        };
        self.ignored.name(&entry.name) || hidden_dir || by_git
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalising() {
        assert_eq!(normalise(".\\docs\\\\a.md"), "docs/a.md");
        assert_eq!(normalise("./a//b"), "a/b");
        assert_eq!(posix_normalize("a/./b/../c"), "a/c");
        assert_eq!(posix_normalize("../a"), "../a");
        assert_eq!(posix_normalize("a/../.."), "..");
        assert_eq!(posix_normalize("/../a/"), "/a/");
        assert_eq!(posix_normalize(""), ".");
        assert_eq!(posix_normalize("a/"), "a/");
    }

    #[test]
    fn the_listing_leaves_out_hidden_directories_and_the_names_it_is_told_to_ignore() {
        let project = crate::git::tests::Temp::new("files");
        for file in ["docs/a.md", "deps/x/b.md", "src/deps", ".git/c.md", "docs/.snap/s.json", ".hidden.md"] {
            project.write(file, file);
        }
        let mut files = DiskFiles::new(project.0.clone());
        assert_eq!(files.list(), [".hidden.md", "deps/x/b.md", "docs/a.md", "src/deps"]);
        files.ignore(&Ignored::new(&["deps".to_string(), "*.md".to_string()], &[]).unwrap());
        assert_eq!(files.list(), Vec::<String>::new());
        files.ignore(&Ignored::new(&["deps".to_string()], &["a.md".to_string()]).unwrap());
        assert_eq!(files.list(), [".hidden.md"]);
        files.ignore(&Ignored::new(&["deps".to_string()], &[".hidden.md".to_string()]).unwrap());
        assert_eq!(files.list(), [".hidden.md", "docs/a.md"]);
    }

    #[test]
    fn the_listing_leaves_out_what_git_ignores() {
        let project = crate::git::tests::Temp::new("files-git");
        for file in ["docs/a.md", "docs/gen/api.md", "docs/b.md", "notes.md"] {
            project.write(file, file);
        }
        let mut files = DiskFiles::new(project.0.clone());
        let by_git = ["docs/gen/", "docs/b.md"].map(str::to_string);
        files.ignore_what_git_ignores(by_git.into_iter().collect());
        assert_eq!(files.list(), ["docs/a.md", "notes.md"]);
    }
}
