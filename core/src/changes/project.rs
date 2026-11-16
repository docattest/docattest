//! A planned project as the comparison reads it, for the head (the working tree, already
//! loaded for the check) and for the base (files read from git, held in memory, planned
//! without running anything).

use crate::adapter::Language;
use crate::api::attestation::Attestation;
use crate::api::errors::{Error, Result};
use crate::config::{CONFIG_FILE, ProjectConfig, parse_config};
use crate::files::memory::MemoryFiles;
use crate::files::project_files::ProjectFiles;
use crate::project::plan::{Plan, plan_project};
use crate::project::state::State;

/// One planned project: what `changes` reads of either side.
#[derive(Clone, Copy)]
pub struct Project<'a> {
    pub files: &'a dyn ProjectFiles,
    pub config: &'a ProjectConfig,
    /// Living pages: matched by `pages`, not frozen, plus sources with doc-comment examples.
    pub living: &'a [String],
    /// Every attestation on every living page, in plan order.
    pub attestations: &'a [Attestation],
    /// The language: everything a comparison knows of it, it asks here.
    pub language: &'a dyn Language,
}

impl Project<'_> {
    /// Whether the adapter calls `file` a source file of its language.
    pub fn is_source(&self, file: &str) -> bool {
        self.language.source_files().is_match(file)
    }
}

/// The working tree, as loaded for the check.
pub fn head_of(state: &State) -> Project<'_> {
    Project {
        files: &state.files,
        config: &state.config,
        living: &state.living,
        attestations: &state.attestations,
        language: &state.adapter,
    }
}

/// A project planned from files in memory.
#[derive(Debug, Clone)]
pub struct Planned {
    pub files: MemoryFiles,
    pub config: ProjectConfig,
    pub living: Vec<String>,
    pub attestations: Vec<Attestation>,
}

impl Planned {
    pub fn project<'a>(&'a self, language: &'a dyn Language) -> Project<'a> {
        Project {
            files: &self.files,
            config: &self.config,
            living: &self.living,
            attestations: &self.attestations,
            language,
        }
    }
}

/// A project in memory, planned as the working tree is: read the config, find every page and
/// plan every attestation. Nothing runs.
pub fn plan_memory(files: MemoryFiles, language: &dyn Language) -> Result<Planned> {
    if !files.exists(CONFIG_FILE) {
        return Err(Error::user(format!("no {CONFIG_FILE} in the in-memory project")));
    }
    let config = parse_config(&files.read(CONFIG_FILE).map_err(Error::user)?)?;
    let Plan { living, attestations, .. } = plan_project(&files, &config, language)?;
    Ok(Planned { files, config, living, attestations })
}
