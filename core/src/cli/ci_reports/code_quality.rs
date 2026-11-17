//! The report as a GitLab Code Quality report: a JSON array of issues, one for each result
//! that fails or errors and each change of `--base`, which a merge request lists with its
//! page and line.

use serde_json::{Value, json};

use super::occurrences;
use super::repository::{Finding, Located, Origin};
use crate::changes::change::Severity as ChangeSeverity;
use crate::json::pretty;
use crate::report::Status;

/// How much an issue matters, in the words of the Code Quality format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Minor,
    Major,
    Critical,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Minor => "minor",
            Severity::Major => "major",
            Severity::Critical => "critical",
        }
    }
}

/// The Code Quality report of the reports, issues in the order they were reported.
pub(super) fn code_quality(projects: &[Located]) -> String {
    let issues: Vec<Issue> = projects.iter().flat_map(Located::findings).filter_map(Issue::of).collect();
    let identities: Vec<String> = issues.iter().map(Issue::identity).collect();
    let json = issues
        .iter()
        .zip(&identities)
        .zip(occurrences(&identities))
        .map(|((issue, identity), occurrence)| {
            issue.to_json(&fingerprint(&format!("{identity}\0{occurrence}")))
        })
        .collect();
    pretty(&Value::Array(json)) + "\n"
}

/// One issue: a result that fails or errors, or a change.
struct Issue<'a> {
    finding: Finding<'a>,
    severity: Severity,
}

impl<'a> Issue<'a> {
    /// A finding as an issue, or none for a result that passes or is skipped.
    fn of(finding: Finding<'a>) -> Option<Issue<'a>> {
        let severity = match finding.origin {
            Origin::Result(Status::Pass | Status::Skipped) => return None,
            Origin::Result(Status::Fail) | Origin::Change(ChangeSeverity::Fail) => Severity::Major,
            Origin::Result(Status::Error) => Severity::Critical,
            Origin::Change(ChangeSeverity::Warning) => Severity::Minor,
        };
        Some(Issue { finding, severity })
    }

    /// What makes this the same finding in another run: its page, its kind and its message,
    /// but not its line, which moves when lines are added above it, nor the lines the message
    /// names on its own page (a cases row's `docs/a.md:12`, a fact's place at the base),
    /// which move with it.
    fn identity(&self) -> String {
        let Finding { path, page, kind, message, .. } = &self.finding;
        format!("{path}\0{kind}\0{}", without_line_numbers(message, page))
    }

    fn to_json(&self, fingerprint: &str) -> Value {
        let Finding { path, line, kind, message, .. } = &self.finding;
        json!({
            "description": format!("docattest {kind}: {message}"),
            "check_name": format!("docattest-{kind}"),
            "fingerprint": fingerprint,
            "severity": self.severity.as_str(),
            "location": { "path": path, "lines": { "begin": line } },
        })
    }
}

/// A 64-bit FNV-1a hash of `text`, in hexadecimal: the same on every platform and in every
/// version, as a fingerprint must be for GitLab to match a finding with an earlier run's.
fn fingerprint(text: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let hash = text.bytes().fold(OFFSET, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME));
    format!("{hash:016x}")
}

/// `message` with the line left out of each place it names on `page`: `docs/a.md:12` is
/// `docs/a.md`, while another place, such as `host.example:8080`, stays as it is.
fn without_line_numbers(message: &str, page: &str) -> String {
    let mut pieces: Vec<&str> = message.split(page).collect();
    for piece in pieces.iter_mut().skip(1) {
        *piece = without_leading_line_number(piece);
    }
    pieces.join(page)
}

/// `text` without the `:12` it starts with, if it does.
fn without_leading_line_number(text: &str) -> &str {
    let Some(number) = text.strip_prefix(':') else { return text };
    let after = number.trim_start_matches(|c: char| c.is_ascii_digit());
    if after.len() < number.len() { after } else { text }
}

#[cfg(test)]
mod tests {
    use super::super::repository::located;
    use super::super::tests::report;
    use super::*;
    use crate::api::attestation::Kind;
    use crate::changes::change::{Change, ChangeType};
    use crate::report::Outcome;
    use crate::report::members::{Checked, MemberReport};

    fn issues_of(checked: &Checked) -> Vec<Value> {
        serde_json::from_str::<Value>(&code_quality(&located(checked, "site/")))
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    #[test]
    fn results_that_fail_or_error_and_every_change_are_issues() {
        let orphan = Change {
            kind: ChangeType::OrphanedClaim,
            page: "docs/a.md".into(),
            line: 7,
            severity: ChangeSeverity::Fail,
            message: "gone".into(),
        };
        let frozen = Change::warning(ChangeType::FrozenPage, "docs/b.md", 1, "frozen".into());
        let top = report(
            vec![
                Outcome::new("docs/a.md", 1, Kind::Fact, Status::Pass, "ok"),
                Outcome::new("docs/a.md", 3, Kind::Fact, Status::Fail, "expected 4, got 3 from N"),
                Outcome::new("docs/a.md", 5, Kind::Example, Status::Skipped, "network is off"),
            ],
            Some(vec![orphan, frozen]),
        );
        let member = report(vec![Outcome::new("docs/a.md", 2, Kind::Test, Status::Error, "no target")], None);
        let checked = Checked {
            top,
            members: vec![MemberReport {
                path: "packages/acme".into(),
                adapter: "go".into(),
                report: member,
            }],
        };
        let found: Vec<(String, u64, String, String)> = issues_of(&checked)
            .iter()
            .map(|issue| {
                let at = |key: &str| issue[key].as_str().unwrap().to_string();
                let path = issue["location"]["path"].as_str().unwrap().to_string();
                (
                    path,
                    issue["location"]["lines"]["begin"].as_u64().unwrap(),
                    at("severity"),
                    at("check_name"),
                )
            })
            .collect();
        let row = |path: &str, line, severity: &str, check: &str| {
            (path.into(), line, severity.into(), check.into())
        };
        assert_eq!(
            found,
            vec![
                row("site/docs/a.md", 3, "major", "docattest-fact"),
                row("site/docs/a.md", 7, "major", "docattest-orphaned-claim"),
                row("site/docs/b.md", 1, "minor", "docattest-frozen-page"),
                row("site/packages/acme/docs/a.md", 2, "critical", "docattest-test"),
            ]
        );
    }

    #[test]
    fn a_fingerprint_stays_when_lines_move_and_differs_for_each_finding() {
        // A cases message names its row's place, which moves with it.
        let stale = |line| {
            let message = format!("docs/a.md:{line} `x`: expected 4, got 3 from N");
            Outcome::new("docs/a.md", line, Kind::Cases, Status::Fail, &message)
        };
        let fingerprints = |lines: &[usize]| -> Vec<String> {
            let checked =
                Checked { top: report(lines.iter().map(|&l| stale(l)).collect(), None), members: vec![] };
            issues_of(&checked)
                .iter()
                .map(|issue| issue["fingerprint"].as_str().unwrap().to_string())
                .collect()
        };
        let before = fingerprints(&[3, 9]);
        assert_eq!(fingerprints(&[5, 11]), before);
        assert_ne!(before[0], before[1]);
        assert_eq!(fingerprint(""), "cbf29ce484222325");
        assert_eq!(fingerprint("a"), "af63dc4c8601ec8c");
    }

    #[test]
    fn a_place_the_message_names_off_its_own_page_is_part_of_the_fingerprint() {
        let fingerprint_of = |message: &str| {
            let failed = Outcome::new("docs/a.md", 3, Kind::Example, Status::Fail, message);
            let checked = Checked { top: report(vec![failed], None), members: vec![] };
            issues_of(&checked)[0]["fingerprint"].as_str().unwrap().to_string()
        };
        let refused = |line, port| fingerprint_of(&format!("docs/a.md:{line}: host.example:{port} refused"));
        assert_ne!(refused(3, 8080), refused(3, 8081));
        assert_eq!(refused(3, 8080), refused(9, 8080));
    }
}
