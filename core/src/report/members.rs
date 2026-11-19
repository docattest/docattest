//! A project checked with its members: their reports one after another, and one verdict.

use serde_json::{Value, json};

use super::{Detail, Report, verdict};

/// The top project, where a member's path is read from.
const TOP: &str = ".";

/// A member's report, and where the member is.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberReport {
    /// From the project that lists it, `/`-separated.
    pub path: String,
    /// The adapter its config names.
    pub adapter: String,
    pub report: Report,
}

/// What `check` found in a project and in each of its members, in the order they were checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub top: Report,
    pub members: Vec<MemberReport>,
}

impl Checked {
    /// 1 when any project's report fails.
    pub fn exit_code(&self) -> i32 {
        self.members.iter().map(|m| m.report.exit_code).fold(self.top.exit_code, i32::max)
    }

    /// The top project's report as `--json` prints it, with the members' under `members`
    /// and the exit code of them all. Without members, exactly the top project's report.
    pub fn to_json(&self) -> Value {
        let mut printed = self.top.to_json();
        if self.members.is_empty() {
            return printed;
        }
        let members: Vec<Value> = self
            .members
            .iter()
            .map(|m| json!({ "path": m.path, "adapter": m.adapter, "report": m.report.to_json() }))
            .collect();
        if let Some(object) = printed.as_object_mut() {
            object.insert("members".into(), Value::Array(members));
            object.insert("exitCode".into(), json!(self.exit_code()));
        }
        printed
    }

    /// The reports as `docattest check` prints them, ending with one verdict for them all.
    /// Without members, exactly the top project's report.
    pub fn format(&self, detail: Detail) -> String {
        if self.members.is_empty() {
            return self.top.format(detail);
        }
        let sections = match detail {
            Detail::Full | Detail::Pages => self.full_sections(detail),
            Detail::Attention => self.attention_sections(),
        };
        sections.join("\n\n")
    }

    /// The top project's report, then each member's under its heading, then the summary.
    fn full_sections(&self, detail: Detail) -> Vec<String> {
        let mut sections = vec![self.top.format(detail)];
        sections.extend(self.members.iter().map(|m| format!("{}\n{}", m.heading(), m.report.format(detail))));
        sections.push(self.summary());
        sections
    }

    /// Only the projects that need attention, the top one first and without a heading; then
    /// the verdict.
    fn attention_sections(&self) -> Vec<String> {
        let top =
            Some(self.top.attention_lines()).filter(|lines| !lines.is_empty()).map(|lines| lines.join("\n"));
        let members = self.members.iter().filter_map(|m| {
            let lines = m.report.attention_lines();
            (!lines.is_empty()).then(|| format!("{}\n{}", m.heading(), lines.join("\n")))
        });
        top.into_iter().chain(members).chain([verdict(self.exit_code()).to_string()]).collect()
    }

    /// How many projects were checked and which failed, the top one as `.`; then the verdict.
    fn summary(&self) -> String {
        let top = (TOP, self.top.exit_code);
        let all =
            std::iter::once(top).chain(self.members.iter().map(|m| (m.path.as_str(), m.report.exit_code)));
        let failed: Vec<&str> = all.filter(|(_, code)| *code != 0).map(|(path, _)| path).collect();
        let named = if failed.is_empty() { String::new() } else { format!(" ({})", failed.join(", ")) };
        format!(
            "projects: {} checked, {} failed{named}\n{}",
            self.members.len() + 1,
            failed.len(),
            verdict(self.exit_code())
        )
    }
}

impl MemberReport {
    /// `member <path> (<adapter>)`, the line a member's report is printed under.
    fn heading(&self) -> String {
        format!("member {} ({})", self.path, self.adapter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::attestation::Kind;
    use crate::report::{Coverage, Outcome, Status};

    fn report(exit_code: i32) -> Report {
        let coverage = Coverage {
            living: 1,
            covered: 0,
            uncovered: vec!["README.md".into()],
            prose: vec![],
            frozen: vec![],
            attestations: 0,
            weak: 0,
            cited: 0,
        };
        Report {
            results: vec![],
            coverage,
            surfaces: vec![],
            snapshots: vec![],
            warnings: vec![],
            exit_code,
            changes: None,
        }
    }

    fn member(path: &str, exit_code: i32) -> MemberReport {
        MemberReport { path: path.into(), adapter: "rust".into(), report: report(exit_code) }
    }

    #[test]
    fn without_members_it_is_the_top_projects_report() {
        let alone = Checked { top: report(1), members: vec![] };
        assert_eq!(alone.format(Detail::Full), report(1).format(Detail::Full));
        assert_eq!(alone.to_json(), report(1).to_json());
        assert_eq!(alone.exit_code(), 1);
    }

    #[test]
    fn any_project_that_fails_fails_them_all() {
        let checked = Checked { top: report(0), members: vec![member("core", 0), member("tools/x", 1)] };
        assert_eq!(checked.exit_code(), 1);
        let text = checked.format(Detail::Full);
        assert!(text.contains("\n\nmember tools/x (rust)\nresults: 0 pass"), "{text}");
        assert!(text.ends_with("\n\nprojects: 3 checked, 1 failed (tools/x)\nFAILED"), "{text}");
        let printed = checked.to_json();
        assert_eq!(printed["exitCode"], json!(1));
        assert_eq!(printed["members"][1]["path"], json!("tools/x"));
        assert_eq!(printed["members"][1]["report"]["exitCode"], json!(1));
        let held = Checked { top: report(0), members: vec![member("core", 0)] };
        assert!(held.format(Detail::Full).ends_with("\n\nprojects: 2 checked, 0 failed\nok"));
        let top = Checked { top: report(1), members: vec![member("core", 0)] };
        assert!(top.format(Detail::Full).ends_with("\n\nprojects: 2 checked, 1 failed (.)\nFAILED"));
    }

    #[test]
    fn quietly_only_the_projects_that_need_attention_are_printed() {
        let failing = |page: &str| {
            let mut failed = report(1);
            failed.results = vec![Outcome::new(page, 1, Kind::Fact, Status::Fail, "expected 4, got 3")];
            failed
        };
        let held = Checked { top: report(0), members: vec![member("core", 0), member("tools/x", 0)] };
        assert_eq!(held.format(Detail::Attention), "ok");
        let one = Checked {
            top: failing("README.md"),
            members: vec![
                member("core", 0),
                MemberReport { report: failing("docs/x.md"), ..member("tools/x", 1) },
            ],
        };
        assert_eq!(
            one.format(Detail::Attention),
            "README.md\n  ✗ L1 fact: expected 4, got 3\n\nmember tools/x (rust)\ndocs/x.md\n  ✗ L1 fact: expected 4, got 3\n\nFAILED"
        );
    }
}
