//! Where a copy of the project goes, and how it is claimed: a hidden directory beside the
//! project's own, `.<name>.docattest-mutate-<worker>-<n>`, beside a lock file of the same
//! name with `.lock` added. The run that made the copy holds the lock for as long as the
//! copy lives, and the system frees it when that run ends, however it ends; so a copy
//! whose lock is free is one a run left behind, and is removed.

use std::fs::{self, File, TryLockError};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::interrupt;

/// A directory beside the project, claimed by its lock; removed when dropped.
pub struct Claim {
    pub dir: PathBuf,
    lock_path: PathBuf,
    /// Held while the claim lives; dropping it frees the lock.
    _lock: File,
}

/// A new, empty directory beside `project` for the copy of worker `worker`. The name is
/// the same from run to run, so that what an adapter keeps for a project by its path (built
/// dependencies) is kept for its copies too; a name another run holds is skipped, and one a
/// run left behind is cleared and taken.
pub fn claim(project: &Path, worker: usize) -> std::io::Result<Claim> {
    let (parent, prefix) = place(project)?;
    for n in 0.. {
        let dir = parent.join(format!("{prefix}{worker}-{n}"));
        let lock_path = lock_of(&dir);
        let Some(lock) = locked(&lock_path)? else { continue };
        interrupt::hold(&[&dir, &lock_path]);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir(&dir)?;
        return Ok(Claim { dir, lock_path, _lock: lock });
    }
    unreachable!("the names beside a project do not run out")
}

/// Remove every copy beside `project` that no run holds, and its lock: what a run that was
/// killed left behind.
pub fn clear_stale(project: &Path) -> std::io::Result<()> {
    let (parent, prefix) = place(project)?;
    for entry in fs::read_dir(&parent)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && !name.ends_with(".lock") {
            remove_unless_held(&parent.join(&name))?;
        }
    }
    Ok(())
}

/// Remove the copy `dir` and its lock, unless a run holds the lock.
fn remove_unless_held(dir: &Path) -> std::io::Result<()> {
    let lock_path = lock_of(dir);
    let Some(lock) = locked(&lock_path)? else { return Ok(()) };
    fs::remove_dir_all(dir)?;
    fs::remove_file(&lock_path)?;
    drop(lock);
    Ok(())
}

impl Drop for Claim {
    fn drop(&mut self) {
        // Links in the copy are removed, never followed: what they lead to is the project's.
        let _ = fs::remove_dir_all(&self.dir);
        let _ = fs::remove_file(&self.lock_path);
        interrupt::release(&[&self.dir, &self.lock_path]);
    }
}

/// The directory copies go in, and the start of every copy's name.
fn place(project: &Path) -> std::io::Result<(PathBuf, String)> {
    let parent =
        project.parent().ok_or_else(|| std::io::Error::other("the project has no directory above it"))?;
    let name = project.file_name().map_or_else(|| "project".into(), |n| n.to_string_lossy().into_owned());
    Ok((parent.to_path_buf(), format!(".{name}.docattest-mutate-")))
}

fn lock_of(dir: &Path) -> PathBuf {
    let mut name = dir.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// The lock at `path`, made if need be and held; None when another run holds it.
fn locked(path: &Path) -> std::io::Result<Option<File>> {
    let file = File::options().create(true).truncate(false).write(true).open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) if e.kind() == ErrorKind::Unsupported => Ok(Some(file)),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    fn copies_beside(project: &Temp) -> Vec<String> {
        let (parent, prefix) = place(&project.0).unwrap();
        let mut names: Vec<String> = fs::read_dir(parent)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(&prefix))
            .map(|name| name[prefix.len()..].to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_claim_is_a_locked_sibling_that_another_claim_skips_and_both_go_when_dropped() {
        let project = Temp::new("sibling");
        let first = claim(&project.0, 0).unwrap();
        let second = claim(&project.0, 0).unwrap();
        assert_eq!(copies_beside(&project), ["0-0", "0-0.lock", "0-1", "0-1.lock"]);
        assert!(first.dir.is_dir() && second.dir.is_dir());
        drop((first, second));
        assert_eq!(copies_beside(&project), Vec::<String>::new());
    }

    #[test]
    fn a_copy_a_killed_run_left_is_cleared_and_one_a_live_run_holds_is_not() {
        let project = Temp::new("sibling-stale");
        let (parent, prefix) = place(&project.0).unwrap();
        fs::create_dir(parent.join(format!("{prefix}3-0"))).unwrap();
        fs::write(parent.join(format!("{prefix}3-0/left.txt")), "x").unwrap();
        let held = claim(&project.0, 1).unwrap();
        clear_stale(&project.0).unwrap();
        assert_eq!(copies_beside(&project), ["1-0", "1-0.lock"]);
        drop(held);
        fs::create_dir(parent.join(format!("{prefix}0-0"))).unwrap();
        let taken = claim(&project.0, 0).unwrap();
        assert_eq!(taken.dir, parent.join(format!("{prefix}0-0")));
        assert!(fs::read_dir(&taken.dir).unwrap().next().is_none(), "what was left in it is gone");
    }

    #[test]
    fn a_claim_is_removed_when_the_code_holding_it_panics() {
        let project = Temp::new("sibling-panic");
        let unwound = std::panic::catch_unwind(|| {
            let _claim = claim(&project.0, 0).unwrap();
            panic!("a bug while mutating");
        });
        assert!(unwound.is_err());
        assert_eq!(copies_beside(&project), Vec::<String>::new());
    }
}
