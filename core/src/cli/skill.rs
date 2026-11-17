//! `skill install`: the agent skill, copied to where the agent's descriptor says the agent looks
//! for it.

use std::path::Path;

use super::args::Options;
use super::{Io, resolve};
use crate::agents::load_agent;
use crate::api::errors::{Error, Result};
use crate::files::text_file;

/// The skill's text: `protocol/agents/SKILL.md` in the repository, carried in the binary.
pub const PACKAGED_SKILL: &str = include_str!("../../protocol/agents/SKILL.md");

/// What installing did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed,
    Unchanged,
    Replaced,
}

/// What to do with a skill file that differs from the packaged one: someone's edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Existing {
    /// Leave it, and refuse (`--force` not given).
    Keep,
    Replace,
}

/// Copy the packaged skill to `path`, the place the agent looks for it. An identical file is
/// left as it is; a different one is replaced only when `existing` says so.
pub fn install_skill(path: &Path, existing_file: Existing) -> Result<(String, InstallOutcome)> {
    let file = path.display().to_string();
    let failed = Error::user;
    let existing = if path.exists() { Some(text_file::read_text(path).map_err(failed)?) } else { None };
    if existing.as_deref() == Some(PACKAGED_SKILL) {
        return Ok((file, InstallOutcome::Unchanged));
    }
    if existing.is_some() && existing_file == Existing::Keep {
        return Err(Error::user(format!(
            "{file} exists and differs from the packaged skill; pass --force to replace it"
        )));
    }
    text_file::write_text(path, PACKAGED_SKILL).map_err(failed)?;
    Ok((file, if existing.is_none() { InstallOutcome::Installed } else { InstallOutcome::Replaced }))
}

/// `os.homedir()`: `HOME`, else the user's home in the password database.
fn home() -> std::path::PathBuf {
    std::env::home_dir().unwrap_or_default()
}

/// `skill install [--agent <name or file>] [--global] [--force]`.
pub(super) fn skill(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    if args.first().map(String::as_str) != Some("install") || args.len() > 1 {
        return Err(Error::user(
            "usage: docattest skill install [--agent <name or file>] [--global] [--force]",
        ));
    }
    let agent = load_agent(options.agent.as_deref(), |given| resolve(&io.cwd, given))?.agent;
    let Some(place) = &agent.skill else {
        return Err(Error::user(format!(
            "agent \"{}\" has no place for a skill: its descriptor names none",
            agent.name
        )));
    };
    let home = home();
    let base = if options.global { &home } else { root };
    let (file, outcome) =
        install_skill(&base.join(place), if options.force { Existing::Replace } else { Existing::Keep })?;
    let said = match outcome {
        InstallOutcome::Installed => "installed",
        InstallOutcome::Replaced => "replaced",
        InstallOutcome::Unchanged => "already up to date:",
    };
    io.out(&format!("docattest skill {said} {file}"));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;
    use std::fs;

    #[test]
    fn installs_leaves_an_identical_copy_and_refuses_to_overwrite_an_edited_one() {
        let base = Temp::new("skill");
        let file = base.0.join("some/skills/docattest/SKILL.md");
        let shown = file.display().to_string();
        assert_eq!(install_skill(&file, Existing::Keep), Ok((shown.clone(), InstallOutcome::Installed)));
        assert_eq!(install_skill(&file, Existing::Keep), Ok((shown.clone(), InstallOutcome::Unchanged)));
        fs::write(&file, "edited").unwrap();
        assert_eq!(
            install_skill(&file, Existing::Keep),
            Err(Error::User(format!(
                "docattest: {shown} exists and differs from the packaged skill; pass --force to replace it"
            )))
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "edited");
        assert_eq!(install_skill(&file, Existing::Replace), Ok((shown, InstallOutcome::Replaced)));
        assert_eq!(fs::read_to_string(&file).unwrap(), PACKAGED_SKILL);
    }

    #[test]
    fn ships_a_skill_with_the_frontmatter_agents_look_for() {
        assert!(
            PACKAGED_SKILL
                .starts_with("---\nname: docattest\ndescription: Use when writing or editing documentation")
        );
        assert!(PACKAGED_SKILL.contains("docattest.config.json"));
    }
}
