//! `agents install`: the baseline that needs nothing of the agent but that it reads `AGENTS.md`.
//! One section, between two markers, says what to run after an edit and before finishing;
//! everything outside the markers is the project's own and is never touched.

use std::path::Path;

use super::Io;
use super::command::docattest_command;
use crate::api::errors::{Error, Result};
use crate::files::text_file;

/// The file coding agents read their instructions from, at the project root.
pub const AGENTS_FILE: &str = "AGENTS.md";

/// The section's text (`protocol/agents/agents-section.md` in the repository, carried in the
/// binary), `{docattest}` standing for the command that runs docattest.
pub const PACKAGED_SECTION: &str = include_str!("../../protocol/agents/agents-section.md");

const BEGIN: &str = "<!-- docattest:begin -->";
const END: &str = "<!-- docattest:end -->";

/// What installing did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionOutcome {
    Added,
    Updated,
    Unchanged,
}

/// The section, markers included, for a project that runs docattest with `command`.
pub fn agents_section(command: &str) -> String {
    let body = PACKAGED_SECTION.replace("{docattest}", command);
    format!("{BEGIN}\n{}\n{END}", body.trim_end_matches(crate::text::is_js_space))
}

/// The first place `needle` stands in `bytes` at or after `from`.
fn find(bytes: &[u8], needle: &str, from: usize) -> Option<usize> {
    let rest = bytes.get(from..)?;
    rest.windows(needle.len()).position(|window| window == needle.as_bytes()).map(|at| at + from)
}

/// Compute what writing the section into `<root>/AGENTS.md` would do, without writing.
pub fn plan_section(root: &Path, section: &str) -> Result<(String, SectionOutcome, Option<Vec<u8>>)> {
    let path = root.join(AGENTS_FILE);
    let file = path.display().to_string();
    if !path.exists() {
        return Ok((file, SectionOutcome::Added, Some(format!("{section}\n").into_bytes())));
    }
    let existing = read_file(&path, &file)?;
    let had_section = find(&existing, BEGIN, 0).is_some();
    let written = with_section(&existing, section, &file)?;
    if written == existing {
        return Ok((file, SectionOutcome::Unchanged, None));
    }
    let outcome = if had_section { SectionOutcome::Updated } else { SectionOutcome::Added };
    Ok((file, outcome, Some(written)))
}

/// Write the section into `<root>/AGENTS.md`: a new file, the end of a file that has no
/// section, or in place of the section that is there. The file is handled as bytes, so what
/// is outside the markers stays exactly as it was.
pub fn install_section(root: &Path, section: &str) -> Result<(String, SectionOutcome)> {
    let (file, outcome, bytes) = plan_section(root, section)?;
    if let Some(bytes) = bytes {
        write_file(&root.join(AGENTS_FILE), &file, &bytes)?;
    }
    Ok((file, outcome))
}

fn write_file(path: &Path, file: &str, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| Error::user(text_file::failure("write", file, &e)))
}

fn read_file(path: &Path, file: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::user(text_file::failure("read", file, &e)))
}

/// The file's bytes with the section in place of the one there, or after what is there.
fn with_section(existing: &[u8], section: &str, file: &str) -> Result<Vec<u8>> {
    match (find(existing, BEGIN, 0), find(existing, END, 0)) {
        (None, None) => Ok(appended(existing, section)),
        (Some(begin), Some(end))
            if begin < end
                && find(existing, BEGIN, begin + 1).is_none()
                && find(existing, END, end + 1).is_none() =>
        {
            Ok([&existing[..begin], section.as_bytes(), &existing[end + END.len()..]].concat())
        }
        _ => Err(Error::user(format!(
            "{file}: the docattest section must be one \"{BEGIN}\" with one \"{END}\" after it; fix the markers and run again"
        ))),
    }
}

/// The section after the file's text, a blank line between them.
fn appended(existing: &[u8], section: &str) -> Vec<u8> {
    let gap: &str = if existing.is_empty() || existing.ends_with(b"\n\n") {
        ""
    } else if existing.ends_with(b"\n") {
        "\n"
    } else {
        "\n\n"
    };
    [existing, format!("{gap}{section}\n").as_bytes()].concat()
}

/// Install (or preview under `--dry-run`) the docattest section of `AGENTS.md` (`init --agents`).
pub(super) fn install_agents(root: &Path, options: &super::args::Options, io: &mut Io) -> Result<String> {
    let section = agents_section(&docattest_command(".")?);
    let (file, outcome) = if options.dry_run {
        let (file, outcome, _) = plan_section(root, &section)?;
        (file, outcome)
    } else {
        install_section(root, &section)?
    };
    let said = match (outcome, options.dry_run) {
        (SectionOutcome::Added, false) => "added to",
        (SectionOutcome::Added, true) => "would add to",
        (SectionOutcome::Updated, false) => "updated in",
        (SectionOutcome::Updated, true) => "would update in",
        (SectionOutcome::Unchanged, _) => "already up to date in",
    };
    if !options.json {
        io.out(&format!("docattest agents: {said} {file}"));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;
    use std::fs;

    #[test]
    fn the_section_names_the_command_the_project_runs() {
        let section = agents_section("./.venv/bin/docattest");
        assert!(section.starts_with("<!-- docattest:begin -->\n## "), "{section}");
        assert!(section.ends_with(".\n<!-- docattest:end -->"), "{section}");
        assert!(section.contains("run `./.venv/bin/docattest impact --file <path>`"));
        assert!(section.contains("run `./.venv/bin/docattest check --quiet`"));
        assert!(!section.contains("{docattest}"));
    }

    #[test]
    fn writes_its_section_and_nothing_outside_the_markers() {
        let root = Temp::new("agents");
        let file = root.0.join(AGENTS_FILE);
        let shown = file.display().to_string();
        let one = agents_section("docattest");
        let two = agents_section("npx --no docattest");
        assert_eq!(install_section(&root.0, &one), Ok((shown.clone(), SectionOutcome::Added)));
        assert_eq!(fs::read_to_string(&file).unwrap(), format!("{one}\n"));
        assert_eq!(install_section(&root.0, &one), Ok((shown.clone(), SectionOutcome::Unchanged)));
        assert_eq!(install_section(&root.0, &two), Ok((shown.clone(), SectionOutcome::Updated)));
        assert_eq!(fs::read_to_string(&file).unwrap(), format!("{two}\n"));

        // Bytes that are not UTF-8, and CRLF line ends, come back as they were.
        let mine: &[u8] = b"# Mine\r\n\xff\xfe notes";
        fs::write(&file, mine).unwrap();
        assert_eq!(install_section(&root.0, &one), Ok((shown.clone(), SectionOutcome::Added)));
        assert_eq!(fs::read(&file).unwrap(), [mine, format!("\n\n{one}\n").as_bytes()].concat());
        fs::write(&file, [mine, format!("\n\n{one}\n\nafter é\n").as_bytes()].concat()).unwrap();
        assert_eq!(install_section(&root.0, &two), Ok((shown.clone(), SectionOutcome::Updated)));
        assert_eq!(fs::read(&file).unwrap(), [mine, format!("\n\n{two}\n\nafter é\n").as_bytes()].concat());
    }

    #[test]
    fn refuses_markers_it_cannot_read_as_one_section() {
        let root = Temp::new("agents");
        let file = root.0.join(AGENTS_FILE);
        let section = agents_section("docattest");
        for broken in [
            "<!-- docattest:begin -->\nno end\n".to_string(),
            "<!-- docattest:end -->\n".to_string(),
            "<!-- docattest:end -->\n<!-- docattest:begin -->\n".to_string(),
            format!("{section}\n{section}\n"),
            format!("{section}\n<!-- docattest:end -->\n"),
        ] {
            fs::write(&file, &broken).unwrap();
            let refused = install_section(&root.0, &section).unwrap_err().to_string();
            assert!(refused.ends_with("fix the markers and run again"), "{refused}");
            assert_eq!(fs::read_to_string(&file).unwrap(), broken);
        }
    }
}
