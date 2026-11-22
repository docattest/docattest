//! Installing the lean, single-language agent skill (`docattest init --skill`).

use std::path::Path;

use super::args::Options;
use super::{Io, resolve};
use crate::agents::load_agent;
use crate::api::errors::{Error, Result};
use crate::files::text_file;

/// The core skill template (`protocol/agents/SKILL.md` in the repository, carried in the binary).
pub const PACKAGED_SKILL: &str = include_str!("../../protocol/agents/SKILL.md");

const TS_NOTES: &str = r#"
## TypeScript specifics

- Blocks: ```` ```ts check ```` (runs) or ```` ```ts compile ```` (type-checks). Setup above `// ---cut---` is hidden.
- Assertions: `assert.equal(a, b)` (`assert` is in scope) or trailing `// Output:` / `// Unordered output:` comments.
- Facts: surface exports and module constants by name; values format as JS `String(v)` (`true`, `3`).
- Citations: `it("title")` inside `describe` blocks, or `it.each` rows by runner title.
- Setup module: `export const cases = { ... }`, `export const facts = { ... }`; use `codeSpans(cell)` from `docattest`.
"#;

const PYTHON_NOTES: &str = r#"
## Python specifics

- Blocks: ```` ```python check ```` (or ```` ```pycon check ```` for `>>>` sessions; tag `continue` to share state) or ```` ```python compile ````.
- Assertions: `assert a == b`, session output, or trailing `# Output:` / `# Unordered output:` comments.
- Facts: surface exports and module constants by name; values format as Python `str(v)` (`True`, `3`).
- Citations: `test_...` functions, classes, `parametrize` ids or `subTest` names.
- Setup module: `cases = { ... }`, `facts = { ... }`; use `code_spans(cell)` from `docattest.cells`.
"#;

const GO_NOTES: &str = r#"
## Go specifics

- Blocks: ```` ```go check ```` (statements, declarations, or `package main`) or ```` ```go compile ````.
- Assertions: trailing `// Output:` / `// Unordered output:` comments, or `if a != b { panic(...) }`.
- Facts: surface exports, package constants, literal `var`s, and `//go:build docattest` helpers; values format via `fmt.Sprint`.
- Citations: `Test...` functions and `t.Run("title", ...)` subtests.
- Setup package: `var Cases = map[string]func(map[string]string) error{...}` and exported fact helpers (`cells.CodeSpans(cell)` from `github.com/docattest/docattest/adapters/go/cells`).
"#;

const RUST_NOTES: &str = r#"
## Rust specifics

- Blocks: ```` ```rust check ```` (`main` body or full `fn main`; prefix hidden setup lines with `# `) or ```` ```rust compile ````.
- Assertions: `assert_eq!(a, b)`, `assert!(...)`, or trailing `// Output:` / `// Unordered output:` comments.
- Facts: surface exports, module constants, and `#[cfg(docattest)]` `pub` items; values format via `Display` (else `Debug`).
- Citations: `#[test]` functions inside `mod tests`, or named `rstest`/`test-case` cases.
- Setup file: `pub const CASES: &[(&str, fn(&HashMap<String, String>) -> Result<(), String>)]` and `pub` fact helpers (`docattest_harness::cells::code_spans(cell)`).
"#;

/// Render the skill text tailored to `adapter` (core + single-language specifics).
pub fn skill_for_adapter(adapter: &str) -> String {
    let extra = match adapter {
        "typescript" => TS_NOTES,
        "python" => PYTHON_NOTES,
        "go" => GO_NOTES,
        "rust" => RUST_NOTES,
        _ => "",
    };
    format!("{}{extra}", PACKAGED_SKILL.trim_end()) + "\n"
}

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

/// Write `content` to `path`, the place the agent looks for its skill. An identical file is
/// left as it is; a different one is replaced only when `existing_file` says so.
pub fn install_skill(
    path: &Path,
    content: &str,
    existing_file: Existing,
) -> Result<(String, InstallOutcome)> {
    let file = path.display().to_string();
    let failed = Error::user;
    let existing = if path.exists() { Some(text_file::read_text(path).map_err(failed)?) } else { None };
    if existing.as_deref() == Some(content) {
        return Ok((file, InstallOutcome::Unchanged));
    }
    if existing.is_some() && existing_file == Existing::Keep {
        return Err(Error::user(format!(
            "{file} exists and differs from the packaged skill; pass --force to replace it"
        )));
    }
    text_file::write_text(path, content).map_err(failed)?;
    Ok((file, if existing.is_none() { InstallOutcome::Installed } else { InstallOutcome::Replaced }))
}

/// `os.homedir()`: `HOME`, else the user's home in the password database.
fn home() -> std::path::PathBuf {
    std::env::home_dir().unwrap_or_default()
}

/// Install the single-language skill for `adapter` (`init --skill`).
pub(super) fn install_project_skill(
    root: &Path,
    adapter: &str,
    options: &Options,
    io: &mut Io,
) -> Result<String> {
    let agent = load_agent(options.agent.as_deref(), |given| resolve(&io.cwd, given))?.agent;
    let Some(place) = &agent.skill else {
        return Err(Error::user(format!(
            "agent \"{}\" has no place for a skill: its descriptor names none",
            agent.name
        )));
    };
    let home = home();
    let base = if options.global { &home } else { root };
    let content = skill_for_adapter(adapter);
    let existing = if options.force { Existing::Replace } else { Existing::Keep };
    let (file, outcome) = install_skill(&base.join(place), &content, existing)?;
    let said = match outcome {
        InstallOutcome::Installed => "installed",
        InstallOutcome::Replaced => "replaced",
        InstallOutcome::Unchanged => "already up to date:",
    };
    if !options.json {
        io.out(&format!("docattest skill {said} {file}"));
    }
    Ok(file)
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
        let go_skill = skill_for_adapter("go");
        assert!(go_skill.contains("## Go specifics"));
        assert!(!go_skill.contains("## TypeScript specifics"));
        assert_eq!(
            install_skill(&file, &go_skill, Existing::Keep),
            Ok((shown.clone(), InstallOutcome::Installed))
        );
        assert_eq!(
            install_skill(&file, &go_skill, Existing::Keep),
            Ok((shown.clone(), InstallOutcome::Unchanged))
        );
        fs::write(&file, "edited").unwrap();
        assert_eq!(
            install_skill(&file, &go_skill, Existing::Keep),
            Err(Error::User(format!(
                "docattest: {shown} exists and differs from the packaged skill; pass --force to replace it"
            )))
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "edited");
        assert_eq!(install_skill(&file, &go_skill, Existing::Replace), Ok((shown, InstallOutcome::Replaced)));
        assert_eq!(fs::read_to_string(&file).unwrap(), go_skill);
    }

    #[test]
    fn ships_a_lean_single_language_skill_with_frontmatter() {
        assert!(
            PACKAGED_SKILL
                .starts_with("---\nname: docattest\ndescription: Use when writing or editing documentation")
        );
        assert!(PACKAGED_SKILL.contains("docattest.config.json"));
        let none_skill = skill_for_adapter("none");
        assert!(!none_skill.contains("## Go specifics"));
        assert!(skill_for_adapter("rust").lines().count() < 75);
    }
}
