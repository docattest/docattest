//! Copying a directory tree, and linking to an entry instead of copying it: what a copy of a
//! project is made with, for a sample page's fixture and for `mutate`.

use std::fs;
use std::path::Path;

/// Copy a directory tree. A symbolic link is made again, to what it points to, so that a
/// link out of the tree (an installed dependency) still reaches the same place.
pub fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
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

/// A symbolic link at `target` to what `source` is or points to. Where links cannot be made
/// without privileges (Windows), a copy stands in for one.
#[cfg(unix)]
pub fn link(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(fs::canonicalize(source)?, target)
}

#[cfg(not(unix))]
pub fn link(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() { copy_tree(source, target) } else { fs::copy(source, target).map(|_| ()) }
}
