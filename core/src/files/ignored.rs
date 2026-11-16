//! What is never the project's own, as the adapter's `hello` states it (PROTOCOL.md §3):
//! `ignoredNames`, globs for one name each, and `ignoredMarkers`, the names of files whose
//! presence marks the directory that holds them.
//!
//! Both are rules about one directory entry, so a walk turns away at the directory itself
//! and never looks at what lies below it.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use super::glob::{GlobMatcher, is_glob};

/// The files and directories a language never counts as the project's own.
#[derive(Debug, Clone, Default)]
pub struct Ignored {
    /// Names without glob syntax, compared as spelled.
    exact: HashSet<String>,
    /// The other names, matched against one path segment.
    globs: Option<Arc<GlobMatcher>>,
    /// Files that mark the directory holding them, by name.
    markers: Vec<String>,
}

impl Ignored {
    /// From the lists of a `hello`. `Err` says which entry cannot be read.
    pub fn new(names: &[String], markers: &[String]) -> Result<Ignored, String> {
        let mut exact = HashSet::new();
        let mut globs = Vec::new();
        for name in names {
            one_name("ignoredNames", name)?;
            if is_glob(name) {
                globs.push(name.clone());
            } else {
                exact.insert(name.clone());
            }
        }
        for marker in markers {
            one_name("ignoredMarkers", marker)?;
        }
        Ok(Ignored { exact, globs: name_matcher(&globs)?, markers: markers.to_vec() })
    }

    /// Whether a file or directory of this name is ignored, wherever it is.
    pub fn name(&self, name: &str) -> bool {
        self.exact.contains(name) || self.globs.as_ref().is_some_and(|g| g.is_match(name))
    }

    /// Whether a file of this name marks its directory as not the project's own.
    pub fn marker(&self, name: &str) -> bool {
        self.markers.iter().any(|m| m == name)
    }

    /// Whether a project-relative path has an ignored name among its segments.
    pub fn path(&self, file: &str) -> bool {
        (!self.exact.is_empty() || self.globs.is_some()) && file.split('/').any(|segment| self.name(segment))
    }

    /// The directories, among those of `paths`, that hold a marker: `a/b` for `a/b/<marker>`.
    /// The project root is never one: a marker there marks nothing.
    pub fn marked<'a>(&self, paths: impl Iterator<Item = &'a str>) -> HashSet<&'a str> {
        if self.markers.is_empty() {
            return HashSet::new();
        }
        paths
            .filter_map(|path| path.rsplit_once('/'))
            .filter(|(_, name)| self.marker(name))
            .map(|(dir, _)| dir)
            .collect()
    }

    /// Whether `file` lies in one of the `marked` directories.
    pub fn under(file: &str, marked: &HashSet<&str>) -> bool {
        !marked.is_empty()
            && file.match_indices('/').any(|(at, _)| marked.contains(file.get(..at).unwrap_or("")))
    }

    /// Whether a project-relative path on disk is ignored: by a name among its segments, or
    /// by a marker in one of the directories above it.
    pub fn on_disk(&self, root: &Path, file: &str) -> bool {
        self.path(file)
            || file.match_indices('/').any(|(at, _)| {
                let dir = root.join(file.get(..at).unwrap_or(""));
                self.markers.iter().any(|marker| dir.join(marker).is_file())
            })
    }
}

/// An entry of `key` must name one file or directory.
fn one_name(key: &str, name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('/') || name.starts_with('!') {
        return Err(format!("\"{key}\" has {name:?}, which is not the name of one file or directory"));
    }
    Ok(())
}

/// One matcher for the `ignoredNames` that are globs, if any are.
fn name_matcher(globs: &[String]) -> Result<Option<Arc<GlobMatcher>>, String> {
    if globs.is_empty() {
        return Ok(None);
    }
    let matcher = GlobMatcher::new(globs)
        .map_err(|why| format!("\"ignoredNames\" has a glob that cannot be read: {why}"))?;
    Ok(Some(Arc::new(matcher)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn names_are_globs_for_one_segment() {
        let ignored = Ignored::new(&strings(&["deps", "*.egg-info", "out"]), &[]).unwrap();
        for path in
            ["deps/x/b.md", "a/out", "a/out/b", "x.egg-info/PKG-INFO", "src/x.egg-info", ".egg-info/a"]
        {
            assert!(ignored.path(path), "{path}");
        }
        for path in ["a/deps.md", "outer/b", "egg-info/x", "a/x.egg-infos"] {
            assert!(!ignored.path(path), "{path}");
        }
        assert!(!Ignored::default().path("deps"));
    }

    #[test]
    fn an_entry_names_one_file_or_directory() {
        for bad in ["a/b", "", "!x", "**/x"] {
            assert!(Ignored::new(&strings(&[bad]), &[]).is_err(), "{bad}");
            assert!(Ignored::new(&[], &strings(&[bad])).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_marker_marks_the_directory_that_holds_it() {
        let ignored = Ignored::new(&[], &strings(&["pyvenv.cfg"])).unwrap();
        let paths = ["pyvenv.cfg", "env/pyvenv.cfg", "env/lib/a.py", "tools/v/pyvenv.cfg", "src/a.py"];
        let marked = ignored.marked(paths.iter().copied());
        assert_eq!(marked, HashSet::from(["env", "tools/v"]));
        assert!(Ignored::under("env/lib/a.py", &marked) && Ignored::under("tools/v/x/y.py", &marked));
        assert!(!Ignored::under("src/a.py", &marked) && !Ignored::under("envy/a.py", &marked));
        assert!(!Ignored::under("pyvenv.cfg", &marked) && !Ignored::under("env", &marked));

        let project = crate::git::tests::Temp::new("marked");
        project.write("env/pyvenv.cfg", "");
        project.write("pyvenv.cfg", "");
        assert!(ignored.on_disk(&project.0, "env/lib/a.py") && ignored.on_disk(&project.0, "env/pyvenv.cfg"));
        assert!(!ignored.on_disk(&project.0, "src/a.py") && !ignored.on_disk(&project.0, "a.py"));
    }
}
