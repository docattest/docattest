//! The console and sample blocks a change reaches. A block runs programs, not exports, so no
//! language can say which code it exercises; the config's `commands` says which files each
//! program is built from, and a change to one of them reaches every block that runs it.

use super::{Found, ImpactReference};
use crate::api::attestation::{Attestation, Body};
use crate::api::errors::Result;
use crate::config::{Command, matcher};
use crate::text::compare_utf16;

/// The program an `md check` block runs: docattest, which checks the sample page.
const SAMPLE_PROGRAM: &str = "docattest";

/// How a report names a command, as a console block writes it: `$ mytool`.
pub fn command_call(name: &str) -> String {
    format!("$ {name}")
}

/// The commands built from a file among `changed` (project-relative), sorted, and each block
/// that runs one, as a reference to its call.
pub(super) fn command_runners(
    commands: &[Command],
    attestations: &[Attestation],
    changed: &[String],
) -> Result<(Vec<String>, Vec<ImpactReference>)> {
    let mut names = Vec::new();
    let mut references = Vec::new();
    for command in commands {
        if !builds(command, changed)? {
            continue;
        }
        names.push(command.name.clone());
        references.extend(attestations.iter().filter(|a| runs(a, &command.name)).map(|block| {
            ImpactReference {
                symbol: command_call(&command.name),
                page: block.page.clone(),
                line: block.line,
                found: Found::Proven(block.kind()),
                claim: block.claim.clone(),
            }
        }));
    }
    names.sort_by(|a, b| compare_utf16(a, b));
    Ok((names, references))
}

/// Whether one of `files` (project-relative) is a file one of `commands` is built from.
pub fn builds_any(commands: &[Command], files: &[String]) -> Result<bool> {
    for command in commands {
        if builds(command, files)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn builds(command: &Command, files: &[String]) -> Result<bool> {
    let built_from = matcher(&command.files)?;
    Ok(files.iter().any(|file| built_from.is_match(file)))
}

fn runs(attestation: &Attestation, name: &str) -> bool {
    match &attestation.body {
        Body::Console(console) => console.steps.iter().any(|step| step.runs(name)),
        Body::Sample(_) => name == SAMPLE_PROGRAM,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::memory::MemoryFiles;
    use crate::files::project_files::ProjectFiles;
    use crate::project::plan::plan_project;

    #[test]
    fn a_change_to_a_command_reaches_the_blocks_that_run_it() {
        let page = [
            "Help is printed.",
            "",
            "```console check",
            "$ ./bin/mytool --help",
            "usage",
            "```",
            "",
            "Admin is separate.",
            "",
            "```console check",
            "$ admin",
            "```",
            "",
            "A sample page holds.",
            "",
            "```md check fixture=fixture",
            "N is `3`. <!-- fact: N -->",
            "```",
            "",
        ]
        .join("\n");
        let files = MemoryFiles::new(vec![
            (
                "docattest.config.json".to_string(),
                r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#.to_string(),
            ),
            ("docs/a.md".to_string(), page),
        ]);
        let config = crate::config::parse_config(&files.read("docattest.config.json").unwrap()).unwrap();
        let attestations =
            plan_project(&files, &config, &crate::adapter::fake::typescript()).unwrap().attestations;
        let commands = vec![
            Command { name: "mytool".into(), files: vec!["src/**/*.ts".into()] },
            Command { name: "admin".into(), files: vec!["admin/*.ts".into()] },
            Command { name: "docattest".into(), files: vec!["src/**/*.ts".into()] },
        ];
        let (names, references) =
            command_runners(&commands, &attestations, &["src/cli/main.ts".to_string()]).unwrap();
        assert_eq!(names, ["docattest", "mytool"]);
        let found: Vec<(&str, usize, &str)> =
            references.iter().map(|r| (r.symbol.as_str(), r.line, r.claim.as_str())).collect();
        assert_eq!(found, [("$ mytool", 3, "Help is printed."), ("$ docattest", 16, "A sample page holds.")]);
        assert!(builds_any(&commands, &["admin/x.ts".to_string()]).unwrap());
        assert!(!builds_any(&commands, &["docs/a.md".to_string()]).unwrap());
    }
}
