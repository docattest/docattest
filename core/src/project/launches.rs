//! How the adapters of a project and of its members are started, for the docattest runs
//! inside its check: a sample page's fixture, and a console block's `docattest`. Without
//! this, a fixture in another language, or a run under a member's `adapterCommand`, would
//! look for an adapter on the PATH and not find the one the check itself used.

use crate::adapter::Launch;
use crate::config::{ProjectConfig, load_config};
use crate::files::source::DiskFiles;

/// The project's own adapter first, then, by language, each member's adapter that its entry
/// gives a command for.
#[derive(Debug, Clone)]
pub struct Launches {
    by_language: Vec<(String, Launch)>,
}

impl Launches {
    /// The project's launch, and its members' given commands. A member whose config cannot
    /// be read is left out: the check of that member reports why.
    pub fn of(files: &DiskFiles, config: &ProjectConfig, own: Launch) -> Launches {
        let mut by_language = vec![(config.adapter.clone(), own)];
        for member in &config.members {
            let Some(words) = &member.adapter_command else { continue };
            let Ok(member_config) = load_config(&DiskFiles::new(files.root().join(&member.path))) else {
                continue;
            };
            if by_language.iter().all(|(language, _)| *language != member_config.adapter) {
                let launch = Launch::of_member(Some(words), files.root());
                by_language.push((member_config.adapter, launch));
            }
        }
        Launches { by_language }
    }

    /// How to start the adapter for `language`: as this project or a member started it,
    /// else found as every adapter is.
    pub fn for_language(&self, language: &str) -> Launch {
        let found = self.by_language.iter().find(|(name, _)| name == language);
        found.map_or(Launch::Found, |(_, launch)| launch.clone())
    }

    /// The variables that let a docattest run inside this one start the same adapters.
    pub fn variables(&self) -> Vec<(String, String)> {
        self.by_language.iter().filter_map(|(language, launch)| launch.variable(language)).collect()
    }
}
