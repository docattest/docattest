//! Checking a sample page: an `md check` block, written into a temporary copy of its fixture
//! project and checked there, as `docattest check <page>` would, with the fixture's own
//! config and adapter.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{Network, run_check};
use crate::api::attestation::{Attestation, Kind, Sample};
use crate::api::errors::{Error, Result, unprefixed};
use crate::files::source::DiskFiles;
use crate::project::state::{State, load_state_beside};
use crate::report::{Outcome, Status};

/// Check the sample page in a copy of its fixture, beside the project being `checked`: at
/// its instant, and with the adapter of the fixture's language started as the checked
/// project or one of its members started one. A problem with the fixture or its project is
/// the block's `error`; only a bug in docattest fails the check.
pub(super) fn check_sample(
    attestation: &Attestation,
    sample: &Sample,
    network: Network,
    checked: &State,
) -> Result<Outcome> {
    let root = checked.files.root();
    let verdict = |status: Status, message: &str| {
        Outcome::new(&attestation.page, attestation.line, Kind::Example, status, message)
    };
    let fixture = &sample.fixture;
    if !root.join(fixture).is_dir() {
        return Ok(verdict(Status::Error, &format!("fixture={fixture} is not a directory of the project")));
    }
    let copy = match Copy::of(&root.join(fixture), &sample.path, &sample.code) {
        Ok(copy) => copy,
        Err(e) => {
            return Ok(verdict(
                Status::Error,
                &format!("cannot copy {fixture} to check the sample page: {e}"),
            ));
        }
    };
    match results_in(&copy.0, &sample.path, network, checked) {
        Ok(Some(results)) => {
            let (status, message) = judge(&results, fixture, attestation.line);
            Ok(verdict(status, &message))
        }
        Ok(None) => Ok(verdict(
            Status::Fail,
            &format!(
                "the sample page {} is not one of the pages of {fixture} (\"pages\" in its docattest.config.json)",
                sample.path
            ),
        )),
        Err(Error::User(message)) => {
            let why = unprefixed(&message);
            Ok(verdict(
                Status::Error,
                &format!("the sample page cannot be checked in a copy of {fixture}: {why}"),
            ))
        }
        Err(bug) => Err(bug),
    }
}

/// The results on `page` of a check of the project at `root`; None when it is not one of
/// the project's living pages.
fn results_in(root: &Path, page: &str, network: Network, checked: &State) -> Result<Option<Vec<Outcome>>> {
    let files = DiskFiles::new(root.to_path_buf());
    let state = load_state_beside(files, checked.now, &checked.launches)?;
    if !state.living.iter().any(|p| p == page) {
        state.adapter.finish()?;
        return Ok(None);
    }
    let checked = run_check(&state, network, &HashSet::from([page.to_string()]));
    state.adapter.finish_session()?;
    Ok(Some(checked?.results.into_iter().filter(|r| r.page == page).collect()))
}

/// The block's status and message from the results on its sample page. A result's line is
/// told as the line of the block's own page it is on: the block's content begins on the
/// line after the fence, at `fence`.
fn judge(results: &[Outcome], fixture: &str, fence: usize) -> (Status, String) {
    let listed = |status: fn(&Outcome) -> bool| -> String {
        let at = |r: &Outcome| format!("L{} {}: {}", fence + r.line, r.kind.as_str(), r.message);
        results.iter().filter(|r| status(r)).map(at).collect::<Vec<_>>().join("; ")
    };
    if results.is_empty() {
        return (Status::Fail, "the sample page has no proof on it".to_string());
    }
    if results.iter().any(Outcome::is_failure) {
        return (
            Status::Fail,
            format!("the sample page fails in a copy of {fixture}: {}", listed(Outcome::is_failure)),
        );
    }
    let skipped = |r: &Outcome| r.status == Status::Skipped;
    if results.iter().any(skipped) {
        return (Status::Skipped, format!("on the sample page, in a copy of {fixture}, {}", listed(skipped)));
    }
    (Status::Pass, format!("every proof on the sample page held in a copy of {fixture}"))
}

/// A temporary copy of a fixture project with the sample page written into it; removed
/// when dropped.
struct Copy(PathBuf);

impl Copy {
    fn of(fixture: &Path, page: &str, text: &str) -> std::io::Result<Copy> {
        let copy = Copy(fresh_directory()?);
        copy_tree(fixture, &copy.0)?;
        let path = copy.0.join(page);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{text}\n"))?;
        Ok(copy)
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        // What is left in the temporary directory is the system's to clear; the check's
        // verdict does not depend on it.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A new, empty directory under the system's temporary directory. Creating it is what
/// claims the name, so two checks never share one.
fn fresh_directory() -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir();
    let mut n = 0;
    loop {
        let dir = base.join(format!("docattest-sample-{}-{n}", std::process::id()));
        match fs::create_dir(&dir) {
            Err(e) if e.kind() == ErrorKind::AlreadyExists => n += 1,
            created => return created.map(|()| dir),
        }
    }
}

/// Copy a directory tree. A symbolic link is made again, to what it points to, so that a
/// link out of the fixture (an installed dependency) still reaches the same place.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        copy_entry(&entry?, to)?;
    }
    Ok(())
}

/// One entry of a directory into `to`: a directory as a tree, a link as a link, else a file.
fn copy_entry(entry: &fs::DirEntry, to: &Path) -> std::io::Result<()> {
    let (source, target) = (entry.path(), to.join(entry.file_name()));
    let kind = entry.file_type()?;
    if kind.is_dir() {
        copy_tree(&source, &target)
    } else if kind.is_symlink() {
        link(&source, &target)
    } else {
        fs::copy(&source, &target).map(|_| ())
    }
}

#[cfg(unix)]
fn link(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(fs::canonicalize(source)?, target)
}

#[cfg(not(unix))]
fn link(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() { copy_tree(source, target) } else { fs::copy(source, target).map(|_| ()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    fn result(line: usize, status: Status, message: &str) -> Outcome {
        Outcome::new("docs/sample.md", line, Kind::Fact, status, message)
    }

    #[test]
    fn a_sample_holds_when_every_proof_on_it_passes() {
        let pass = result(1, Status::Pass, "N is 3");
        assert_eq!(
            judge(std::slice::from_ref(&pass), "f", 10),
            (Status::Pass, "every proof on the sample page held in a copy of f".into())
        );
        assert_eq!(judge(&[], "f", 10), (Status::Fail, "the sample page has no proof on it".into()));
        let failures = [
            result(2, Status::Fail, "expected 4, got 3"),
            pass.clone(),
            result(5, Status::Error, "no target"),
        ];
        assert_eq!(
            judge(&failures, "f", 10),
            (
                Status::Fail,
                "the sample page fails in a copy of f: L12 fact: expected 4, got 3; L15 fact: no target"
                    .into()
            )
        );
        let skipped = [pass, result(3, Status::Skipped, "network is off")];
        assert_eq!(
            judge(&skipped, "f", 10),
            (Status::Skipped, "on the sample page, in a copy of f, L13 fact: network is off".into())
        );
    }

    #[test]
    fn the_copy_holds_the_fixture_and_the_page_and_goes_when_dropped() {
        let fixture = Temp::new("sample-fixture");
        fixture.write("docattest.config.json", "{}");
        fixture.write("src/a.ts", "x");
        let copy = Copy::of(&fixture.0, "docs/sample.md", "# A").unwrap();
        let read = |rel: &str| fs::read_to_string(copy.0.join(rel)).unwrap();
        assert_eq!(
            (read("docattest.config.json"), read("src/a.ts"), read("docs/sample.md")),
            ("{}".into(), "x".into(), "# A\n".into())
        );
        let place = copy.0.clone();
        drop(copy);
        assert!(!place.exists());
    }
}
