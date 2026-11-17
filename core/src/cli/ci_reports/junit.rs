//! The report as JUnit XML, the test report most CI services show (GitLab, Jenkins, CircleCI,
//! Azure Pipelines): a test suite for each page, a test case for each result, and a failing
//! test case for each change of `--base` that fails. JUnit has no warnings, so a change that
//! only warns is left out.

use std::collections::HashMap;

use super::occurrences;
use super::repository::{Finding, Located, Origin};
use crate::changes::change::Severity;
use crate::report::Status;

/// The JUnit XML of the reports, suites in the order their pages were first reported.
pub(super) fn junit(projects: &[Located]) -> String {
    let suites = suites(projects.iter().flat_map(Located::findings).filter_map(Case::of));
    let all = Counts::of(suites.iter().flat_map(|(_, cases)| cases));
    let mut lines = vec![
        r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string(),
        format!(r#"<testsuites name="docattest"{}>"#, all.attributes()),
    ];
    for (page, cases) in &suites {
        lines.push(format!(r#"  <testsuite name="{}"{}>"#, attribute(page), Counts::of(cases).attributes()));
        lines.extend(cases.iter().zip(names(cases)).map(|(case, name)| case.xml(&name)));
        lines.push("  </testsuite>".into());
    }
    lines.push("</testsuites>".into());
    lines.join("\n") + "\n"
}

/// One test case: a result, or a change that fails.
struct Case<'a> {
    finding: Finding<'a>,
    status: Status,
}

/// The cases grouped by page, pages in the order they first come.
fn suites<'a>(cases: impl Iterator<Item = Case<'a>>) -> Vec<(String, Vec<Case<'a>>)> {
    let mut suites: Vec<(String, Vec<Case>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for case in cases {
        let page = &case.finding.path;
        let at = *index.entry(page.clone()).or_insert_with(|| {
            suites.push((page.clone(), Vec::new()));
            suites.len() - 1
        });
        suites[at].1.push(case);
    }
    suites
}

/// Each case's name in its suite, `L3 fact`: a CI service keys a test by its suite and name,
/// so a second result of the same kind on the same line is `L3 fact (2)`.
fn names(cases: &[Case]) -> Vec<String> {
    let names: Vec<String> =
        cases.iter().map(|case| format!("L{} {}", case.finding.line, case.finding.kind)).collect();
    let counts = occurrences(&names);
    names
        .into_iter()
        .zip(counts)
        .map(|(name, count)| if count == 1 { name } else { format!("{name} ({count})") })
        .collect()
}

impl<'a> Case<'a> {
    /// A finding as a test case, or none for a change that only warns: JUnit has no warnings.
    fn of(finding: Finding<'a>) -> Option<Case<'a>> {
        let status = match finding.origin {
            Origin::Result(status) => status,
            Origin::Change(Severity::Fail) => Status::Fail,
            Origin::Change(Severity::Warning) => return None,
        };
        Some(Case { finding, status })
    }

    /// The `<testcase>` element named `name`, indented to sit in its suite.
    fn xml(&self, name: &str) -> String {
        let element = match self.status {
            Status::Pass => None,
            Status::Skipped => Some(format!(r#"<skipped message="{}"/>"#, attribute(self.finding.message))),
            Status::Fail => Some(self.problem("failure")),
            Status::Error => Some(self.problem("error")),
        };
        let page = attribute(&self.finding.path);
        let open = format!(
            r#"    <testcase name="{}" classname="{page}" file="{page}" line="{}""#,
            attribute(name),
            self.finding.line
        );
        match element {
            None => format!("{open}/>"),
            Some(element) => format!("{open}>\n      {element}\n    </testcase>"),
        }
    }

    /// `<failure>` or `<error>`: the message and the kind, and as its text the message with
    /// what was expected and what was found when the result says.
    fn problem(&self, tag: &str) -> String {
        let Finding { message, kind, expected, actual, .. } = &self.finding;
        let detail = match (expected, actual) {
            (Some(expected), Some(actual)) => {
                format!("{message}\n\nexpected:\n{expected}\n\nactual:\n{actual}")
            }
            _ => message.to_string(),
        };
        format!(r#"<{tag} message="{}" type="{kind}">{}</{tag}>"#, attribute(message), text(&detail))
    }
}

/// How many cases there are, and how many of them ended each way but passing.
struct Counts {
    tests: usize,
    failures: usize,
    errors: usize,
    skipped: usize,
}

impl Counts {
    fn of<'a>(cases: impl IntoIterator<Item = &'a Case<'a>>) -> Counts {
        let mut counts = Counts { tests: 0, failures: 0, errors: 0, skipped: 0 };
        for case in cases {
            counts.tests += 1;
            match case.status {
                Status::Pass => {}
                Status::Fail => counts.failures += 1,
                Status::Error => counts.errors += 1,
                Status::Skipped => counts.skipped += 1,
            }
        }
        counts
    }

    /// The counts as the attributes of a suite.
    fn attributes(&self) -> String {
        let Counts { tests, failures, errors, skipped } = self;
        format!(r#" tests="{tests}" failures="{failures}" errors="{errors}" skipped="{skipped}""#)
    }
}

/// `value` escaped for an attribute in double quotes. Its line breaks and tabs are written
/// as references, because a parser turns them into spaces otherwise.
fn attribute(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '"' => "&quot;".to_string(),
            '\n' => "&#10;".to_string(),
            '\r' => "&#13;".to_string(),
            '\t' => "&#9;".to_string(),
            other => escaped(other),
        })
        .collect()
}

/// `value` escaped as the text of an element.
fn text(value: &str) -> String {
    value.chars().map(escaped).collect()
}

/// One character as XML 1.0 can hold it. A control character other than a line break or a
/// tab cannot be written at all, even as a reference (a console's colour codes are some),
/// so it is replaced.
fn escaped(c: char) -> String {
    match c {
        '&' => "&amp;".into(),
        '<' => "&lt;".into(),
        '>' => "&gt;".into(),
        '\t' | '\n' | '\r' => c.into(),
        c if c < ' ' || c == '\u{FFFE}' || c == '\u{FFFF}' => char::REPLACEMENT_CHARACTER.into(),
        c => c.into(),
    }
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

    #[test]
    fn each_result_is_a_test_case_in_its_pages_suite() {
        let top = report(
            vec![
                Outcome::new("docs/a.md", 1, Kind::Fact, Status::Pass, "ok"),
                Outcome::new("docs/a.md", 3, Kind::Fact, Status::Fail, "expected 4, got 3 from N")
                    .with_comparison("4", "3"),
                Outcome::new("docs/b.md", 2, Kind::Example, Status::Skipped, "network is off"),
            ],
            None,
        );
        let member = report(
            vec![Outcome::new("docs/a.md", 5, Kind::Test, Status::Error, "no such <file> & \u{1b}[31m\"x\"")],
            None,
        );
        let checked = Checked {
            top,
            members: vec![MemberReport {
                path: "packages/acme".into(),
                adapter: "rust".into(),
                report: member,
            }],
        };
        assert_eq!(
            junit(&located(&checked, "site/")),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="docattest" tests="4" failures="1" errors="1" skipped="1">
  <testsuite name="site/docs/a.md" tests="2" failures="1" errors="0" skipped="0">
    <testcase name="L1 fact" classname="site/docs/a.md" file="site/docs/a.md" line="1"/>
    <testcase name="L3 fact" classname="site/docs/a.md" file="site/docs/a.md" line="3">
      <failure message="expected 4, got 3 from N" type="fact">expected 4, got 3 from N

expected:
4

actual:
3</failure>
    </testcase>
  </testsuite>
  <testsuite name="site/docs/b.md" tests="1" failures="0" errors="0" skipped="1">
    <testcase name="L2 example" classname="site/docs/b.md" file="site/docs/b.md" line="2">
      <skipped message="network is off"/>
    </testcase>
  </testsuite>
  <testsuite name="site/packages/acme/docs/a.md" tests="1" failures="0" errors="1" skipped="0">
    <testcase name="L5 test" classname="site/packages/acme/docs/a.md" file="site/packages/acme/docs/a.md" line="5">
      <error message="no such &lt;file&gt; &amp; �[31m&quot;x&quot;" type="test">no such &lt;file&gt; &amp; �[31m"x"</error>
    </testcase>
  </testsuite>
</testsuites>
"#
        );
    }

    #[test]
    fn a_change_that_fails_is_a_failing_test_case_and_a_warning_is_left_out() {
        let orphan = Change {
            kind: ChangeType::OrphanedClaim,
            page: "docs/a.md".into(),
            line: 3,
            severity: Severity::Fail,
            message: "the fact that proved it is gone".into(),
        };
        let frozen = Change::warning(ChangeType::FrozenPage, "docs/b.md", 1, "frozen".into());
        let checked = Checked { top: report(vec![], Some(vec![orphan, frozen])), members: vec![] };
        let xml = junit(&located(&checked, ""));
        assert!(
            xml.contains(r#"<testsuites name="docattest" tests="1" failures="1" errors="0" skipped="0">"#)
        );
        assert!(xml.contains(
            r#"<failure message="the fact that proved it is gone" type="orphaned-claim">the fact that proved it is gone</failure>"#
        ));
        assert!(!xml.contains("frozen"), "{xml}");
    }

    #[test]
    fn results_of_one_kind_on_one_line_are_told_apart_by_their_names() {
        let pass = |page| Outcome::new(page, 3, Kind::Fact, Status::Pass, "ok");
        let checked = Checked {
            top: report(vec![pass("docs/a.md"), pass("docs/a.md"), pass("docs/b.md")], None),
            members: vec![],
        };
        let xml = junit(&located(&checked, ""));
        let names: Vec<&str> =
            xml.match_indices(r#"name="L"#).map(|(at, _)| xml[at + 6..].split('"').next().unwrap()).collect();
        assert_eq!(names, ["L3 fact", "L3 fact (2)", "L3 fact"]);
    }
}
