//! A project held in memory: the base of a `check --base`, read from git, or a test's.

use std::collections::HashMap;

use super::project_files::ProjectFiles;
use super::source::normalise;
use crate::text::compare_utf16;

/// A project held in memory: path → contents.
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    files: HashMap<String, String>,
    /// Every path, sorted by UTF-16 code units.
    sorted: Vec<String>,
}

impl MemoryFiles {
    /// From path and contents pairs; a path given twice keeps its last contents.
    pub fn new(entries: impl IntoIterator<Item = (String, String)>) -> MemoryFiles {
        let files: HashMap<String, String> = entries.into_iter().map(|(k, v)| (normalise(&k), v)).collect();
        let mut sorted: Vec<String> = files.keys().cloned().collect();
        sorted.sort_by(|a, b| compare_utf16(a, b));
        MemoryFiles { files, sorted }
    }
}

impl ProjectFiles for MemoryFiles {
    fn read(&self, file: &str) -> Result<String, String> {
        self.files
            .get(&normalise(file))
            .cloned()
            .ok_or_else(|| format!("cannot read {file}: there is no such file"))
    }

    fn exists(&self, file: &str) -> bool {
        self.files.contains_key(&normalise(file))
    }

    fn list(&self) -> Vec<String> {
        self.sorted.clone()
    }
}
