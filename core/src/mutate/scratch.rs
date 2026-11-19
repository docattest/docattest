//! A copy of the project in which `mutate` changes code, one change at a time, so that the
//! project itself is never written. It is a sibling of the project's directory, so that a
//! path that leads out of the project (`../shared`, a parent's `node_modules` or workspace)
//! leads to the same place from the copy. It holds a copy of each of the project's own
//! files; every other entry (an installed dependency, a build, what git ignores, a hidden
//! directory) is a link to the original, which a change is never made through.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::sibling::{Claim, claim};
use crate::files::copy::link;

/// A copy of a project beside it; removed when dropped.
pub struct Scratch {
    /// The directory beside the project, claimed for this copy.
    place: Claim,
    /// The project's own files, which alone are copied and may be changed.
    own: HashSet<String>,
    /// The modification time the last write gave a file.
    written_at: SystemTime,
}

impl Scratch {
    /// A copy of the project at `project`, whose own files are `own` (project-relative).
    /// `worker` tells copies made at the same time apart. Every file copied is given the
    /// time the copy was made: a copy at this path may have been built before, from a
    /// change made then, and a build system must not take that build for one of this copy.
    pub fn beside(project: &Path, own: &[String], worker: usize) -> std::io::Result<Scratch> {
        let place = claim(project, worker)?;
        let scratch =
            Scratch { place, own: own.iter().cloned().collect(), written_at: next_second(SystemTime::now()) };
        let ancestors = ancestors_of(own);
        copy_dir(project, &scratch.place.dir, "", &scratch.own, &ancestors)?;
        for file in own {
            stamp(&scratch.place.dir.join(file), scratch.written_at)?;
        }
        Ok(scratch)
    }

    pub fn root(&self) -> &Path {
        &self.place.dir
    }

    /// Whether `file` is one of the project's own files, which the copy holds and may change.
    pub fn holds(&self, file: &str) -> bool {
        self.own.contains(file)
    }

    /// Write `text` as `file`, one of the project's own files. Its modification time is a
    /// whole second later than any version of a file the copy held before, so that no tool
    /// that keeps what it built by a source's time (a bytecode cache, a build system) takes
    /// one version for another, however fast they follow each other.
    pub fn write(&mut self, file: &str, text: &str) -> std::io::Result<()> {
        assert!(self.holds(file), "{file} is not a file the copy may change");
        let path = self.place.dir.join(file);
        fs::write(&path, text)?;
        self.written_at += Duration::from_secs(1);
        stamp(&path, self.written_at)
    }
}

/// Give the file at `path` the modification time `at`.
fn stamp(path: &Path, at: SystemTime) -> std::io::Result<()> {
    fs::File::options().write(true).open(path)?.set_modified(at)
}

/// The start of the next whole second after `now`.
fn next_second(now: SystemTime) -> SystemTime {
    let since = now.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    SystemTime::UNIX_EPOCH + Duration::from_secs(since.as_secs() + 1)
}

/// Every directory that holds one of `own`, at any depth, as a project-relative path.
fn ancestors_of(own: &[String]) -> HashSet<String> {
    let mut out = HashSet::new();
    for file in own {
        let mut at = file.as_str();
        while let Some(slash) = at.rfind('/') {
            at = &at[..slash];
            out.insert(at.to_string());
        }
    }
    out
}

/// The entries of `from` (at `rel` in the project) into `to`: an own file copied, a directory
/// that holds own files made again, anything else linked.
fn copy_dir(
    from: &Path,
    to: &Path,
    rel: &str,
    own: &HashSet<String>,
    ancestors: &HashSet<String>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_dir() && ancestors.contains(&child) {
            fs::create_dir(&target)?;
            copy_dir(&source, &target, &child, own, ancestors)?;
        } else if kind.is_file() && own.contains(&child) {
            fs::copy(&source, &target)?;
        } else {
            link(&source, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    #[test]
    fn the_copy_is_a_sibling_with_own_files_copied_and_the_rest_linked() {
        let project = Temp::new("scratch");
        for file in ["src/a.ts", "docs/a.md", "node_modules/dep/index.js", ".git/HEAD", "src/gen/out.js"] {
            project.write(file, file);
        }
        let own = ["docs/a.md".to_string(), "src/a.ts".to_string()];
        let mut scratch = Scratch::beside(&project.0, &own, 3).unwrap();
        assert_eq!(scratch.root().parent(), project.0.parent());
        let root = scratch.root().to_path_buf();
        let read = |rel: &str| fs::read_to_string(root.join(rel)).unwrap();
        assert_eq!(
            (read("src/a.ts"), read("node_modules/dep/index.js")),
            ("src/a.ts".into(), "node_modules/dep/index.js".into())
        );
        assert_eq!(read("src/gen/out.js"), "src/gen/out.js");
        #[cfg(unix)]
        for linked in ["node_modules", ".git", "src/gen"] {
            assert!(fs::symlink_metadata(scratch.root().join(linked)).unwrap().is_symlink(), "{linked}");
        }
        scratch.write("src/a.ts", "changed").unwrap();
        assert_eq!(read("src/a.ts"), "changed");
        assert_eq!(fs::read_to_string(project.0.join("src/a.ts")).unwrap(), "src/a.ts");
        let place = scratch.root().to_path_buf();
        drop(scratch);
        assert!(!place.exists());
        assert!(project.0.join("node_modules/dep/index.js").exists());
    }

    #[test]
    fn each_write_is_a_second_later_than_the_last() {
        let project = Temp::new("scratch-times");
        project.write("a.py", "N = 1\n");
        let mut scratch = Scratch::beside(&project.0, &["a.py".to_string()], 0).unwrap();
        let modified = |s: &Scratch| fs::metadata(s.root().join("a.py")).unwrap().modified().unwrap();
        let copied = modified(&scratch);
        assert!(copied > fs::metadata(project.0.join("a.py")).unwrap().modified().unwrap());
        scratch.write("a.py", "N = 2\n").unwrap();
        let first = modified(&scratch);
        assert_eq!(first.duration_since(copied).unwrap(), Duration::from_secs(1));
        scratch.write("a.py", "N = 1\n").unwrap();
        assert_eq!(modified(&scratch).duration_since(first).unwrap(), Duration::from_secs(1));
    }

    #[test]
    #[should_panic(expected = "node_modules/x.js is not a file the copy may change")]
    fn a_file_that_is_not_the_projects_own_is_never_written() {
        let project = Temp::new("scratch-own");
        project.write("node_modules/x.js", "x");
        let mut scratch = Scratch::beside(&project.0, &[], 0).unwrap();
        let _ = scratch.write("node_modules/x.js", "changed");
    }
}
