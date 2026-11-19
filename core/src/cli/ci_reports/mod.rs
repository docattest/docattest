//! `check --report <format>=<path>`: the report written again, to a file in a format a CI
//! service reads, beside what `check` prints. Each format has a writer of its own, given the
//! same reports `--json` prints; every path they write is from the top of the git
//! repository, which is where a CI service looks for a file.

mod code_quality;
mod junit;
mod repository;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::api::errors::{Error, Result};
use crate::report::members::Checked;

/// A format a CI service reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Junit,
    CodeQuality,
}

/// The formats by the name `--report` gives them.
const FORMATS: [(&str, Format); 2] = [("junit", Format::Junit), ("codequality", Format::CodeQuality)];

/// One file `--report` asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportFile {
    format: Format,
    path: PathBuf,
}

/// The files `--report` asked for, and where the project is in its repository.
pub(super) struct ReportFiles {
    files: Vec<ReportFile>,
    /// The project root's directories below the top of the repository, each followed by `/`.
    /// Asked of git only when there are files to write.
    prefix: String,
}

impl ReportFiles {
    /// The files asked for, each `<format>=<path>` with a relative path from `cwd`. Read
    /// before the check runs, so a mistake fails before anything runs.
    pub(super) fn asked(given: &[String], cwd: &Path, root: &Path) -> Result<ReportFiles> {
        let files = given.iter().map(|one| report_file(one, cwd)).collect::<Result<Vec<_>>>()?;
        let prefix = if files.is_empty() { String::new() } else { repository::prefix(root)? };
        Ok(ReportFiles { files, prefix })
    }

    /// Writes each file, with the reports of the project and of its members.
    pub(super) fn write(&self, checked: &Checked) -> Result<()> {
        let projects = repository::located(checked, &self.prefix);
        for file in &self.files {
            let text = match file.format {
                Format::Junit => junit::junit(&projects),
                Format::CodeQuality => code_quality::code_quality(&projects),
            };
            write_file(&file.path, &text)?;
        }
        Ok(())
    }
}

fn report_file(given: &str, cwd: &Path) -> Result<ReportFile> {
    let format = |name: &str| FORMATS.iter().find(|(known, _)| *known == name).map(|&(_, format)| format);
    let asked = given.split_once('=').filter(|(_, path)| !path.is_empty());
    let Some((format, path)) = asked.and_then(|(name, path)| Some((format(name)?, path))) else {
        let names: Vec<&str> = FORMATS.iter().map(|(name, _)| *name).collect();
        return Err(Error::user(format!(
            "--report takes <format>=<path>, where <format> is {}, not \"{given}\"",
            names.join(" or ")
        )));
    };
    Ok(ReportFile { format, path: super::resolve(cwd, path) })
}

/// For each key, how many times it has come so far, itself included: 1 the first time, 2
/// the first time it repeats. A CI service keys a finding by what a format writes of it, so a
/// repeat is told apart by this count.
fn occurrences(keys: &[String]) -> Vec<usize> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    keys.iter()
        .map(|key| {
            let count = seen.entry(key).or_default();
            *count += 1;
            *count
        })
        .collect()
}

fn write_file(path: &Path, text: &str) -> Result<()> {
    let made = path.parent().map_or(Ok(()), std::fs::create_dir_all);
    made.and_then(|()| std::fs::write(path, text))
        .map_err(|why| Error::user(format!("could not write {}: {why}", path.display())))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::changes::Change;
    use crate::report::{Coverage, Outcome, Report};

    /// A report of these results and, as with `--base`, these changes.
    pub(super) fn report(results: Vec<Outcome>, changes: Option<Vec<Change>>) -> Report {
        let coverage = Coverage {
            living: 1,
            covered: 1,
            uncovered: vec![],
            prose: vec![],
            frozen: vec![],
            attestations: results.len(),
            weak: 0,
            cited: 0,
        };
        let exit_code = crate::report::exit_code_for(&results);
        Report {
            results,
            coverage,
            surfaces: vec![],
            snapshots: vec![],
            exit_code,
            changes,
            warnings: vec![],
        }
    }

    #[test]
    fn a_report_file_is_a_known_format_and_a_path_from_the_working_directory() {
        let cwd = Path::new("/work/site");
        let file = report_file("codequality=out/gl.json", cwd).unwrap();
        assert_eq!(
            file,
            ReportFile { format: Format::CodeQuality, path: PathBuf::from("/work/site/out/gl.json") }
        );
        let refused = |given: &str| report_file(given, cwd).unwrap_err().to_string();
        let wanted = "docattest: --report takes <format>=<path>, where <format> is junit or codequality";
        assert_eq!(refused("xml=a.xml"), format!("{wanted}, not \"xml=a.xml\""));
        assert_eq!(refused("junit"), format!("{wanted}, not \"junit\""));
        assert_eq!(refused("junit="), format!("{wanted}, not \"junit=\""));
    }
}
