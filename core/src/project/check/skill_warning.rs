//! Warnings for project-local agent skill files that are out of date with this docattest.

use std::collections::HashSet;

use crate::agents::shipped_agents;
use crate::cli::skill::PACKAGED_SKILL;
use crate::files::project_files::ProjectFiles;

/// Frontmatter header every packaged docattest skill starts with.
const SKILL_PREFIX: &str = "---\nname: docattest\n";

/// A warning for each shipped agent's project-local skill file that exists on disk, carries
/// docattest's skill frontmatter, and differs from the skill packaged in this docattest.
pub(super) fn stale_skill_warnings(files: &dyn ProjectFiles) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut warnings = Vec::new();
    for (agent, is_default) in shipped_agents() {
        let Some(place) = agent.skill else { continue };
        if !seen.insert(place.clone()) || !files.exists(&place) {
            continue;
        }
        let Ok(text) = files.read(&place) else { continue };
        if text != PACKAGED_SKILL && text.starts_with(SKILL_PREFIX) {
            let flag = if is_default { String::new() } else { format!(" --agent {}", agent.name) };
            warnings.push(format!(
                "{place} is out of date with this docattest: run `docattest skill install{flag} --force`"
            ));
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::memory::MemoryFiles;

    #[test]
    fn warns_only_when_a_shipped_agents_installed_skill_is_out_of_date() {
        let (agent, is_default) = shipped_agents().into_iter().find(|(a, _)| a.skill.is_some()).unwrap();
        let path = agent.skill.unwrap();
        assert!(stale_skill_warnings(&MemoryFiles::new([])).is_empty());
        let current = MemoryFiles::new([(path.clone(), PACKAGED_SKILL.to_string())]);
        assert!(stale_skill_warnings(&current).is_empty());
        let unrelated = MemoryFiles::new([(path.clone(), "# My own notes\n".to_string())]);
        assert!(stale_skill_warnings(&unrelated).is_empty());
        let stale = MemoryFiles::new([(path.clone(), format!("{SKILL_PREFIX}description: old\n---\n"))]);
        let flag = if is_default { String::new() } else { format!(" --agent {}", agent.name) };
        assert_eq!(
            stale_skill_warnings(&stale),
            [format!(
                "{path} is out of date with this docattest: run `docattest skill install{flag} --force`"
            )]
        );
    }
}
