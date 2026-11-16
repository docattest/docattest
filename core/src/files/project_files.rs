//! The files of a project, wherever they are: on disk, or in memory for the base of a
//! `check --base`. Planning and comparing read a project only through this.

/// Read access to a project's files, by project-relative path.
pub trait ProjectFiles: Sync {
    /// The text of a file, or why it cannot be read (there is no such file, or the system
    /// refused), worded for the user.
    fn read(&self, file: &str) -> Result<String, String>;
    /// Whether a file is there.
    fn exists(&self, file: &str) -> bool;
    /// Every file of the project, sorted by UTF-16 code units.
    fn list(&self) -> Vec<String>;
}
