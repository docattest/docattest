//! Where a check's pages are in its git repository. A report names each page from the
//! project it belongs to; a CI service finds a file from the top of the repository.

use std::path::Path;

use crate::api::errors::{Error, Result};
use crate::changes::change::Severity;
use crate::files::source::{normalise, posix_normalize};
use crate::git::Failure;
use crate::report::members::Checked;
use crate::report::{Report, Status};

/// One project's report, and the directory it was checked in, from the top of the repository.
pub(super) struct Located<'a> {
    directory: String,
    report: &'a Report,
}

/// A result or a change of `--base`, as every format writes it.
pub(super) struct Finding<'a> {
    /// From the top of the repository.
    pub(super) path: String,
    /// From the project that reported it, which is how its message names it.
    pub(super) page: &'a str,
    pub(super) line: usize,
    /// The kind of attestation, or the type of change.
    pub(super) kind: &'static str,
    pub(super) message: &'a str,
    pub(super) expected: Option<&'a str>,
    pub(super) actual: Option<&'a str>,
    pub(super) origin: Origin,
}

/// What a finding is: a result with its status, or a change with its severity.
#[derive(Clone, Copy)]
pub(super) enum Origin {
    Result(Status),
    Change(Severity),
}

impl<'a> Located<'a> {
    /// Each result of the project, then each change of `--base`.
    pub(super) fn findings(&self) -> Vec<Finding<'a>> {
        let results = self.report.results.iter().map(|r| Finding {
            path: self.path(&r.page),
            page: &r.page,
            line: r.line,
            kind: r.kind.as_str(),
            message: &r.message,
            expected: r.expected.as_deref(),
            actual: r.actual.as_deref(),
            origin: Origin::Result(r.status),
        });
        let changes = self.report.changes.iter().flatten().map(|c| Finding {
            path: self.path(&c.page),
            page: &c.page,
            line: c.line,
            kind: c.kind.as_str(),
            message: &c.message,
            expected: None,
            actual: None,
            origin: Origin::Change(c.severity),
        });
        results.chain(changes).collect()
    }

    /// A page of this project, from the top of the repository.
    fn path(&self, page: &str) -> String {
        let joined =
            if self.directory.is_empty() { page.to_string() } else { format!("{}/{page}", self.directory) };
        posix_normalize(&normalise(&joined))
    }
}

/// Where `root` is below the top of the repository that holds it, as git says.
pub(super) fn prefix(root: &Path) -> Result<String> {
    const WHY: &str = "--report writes each path from the top of the git repository";
    crate::git::prefix(root).map_err(|failure| match failure {
        Failure::Missing => Error::user(format!("{WHY}, and git was not found")),
        Failure::Failed(_) => Error::user(format!("{WHY}, and {} is not in one", root.display())),
    })
}

/// The top project's report and each member's, in the order they were checked, each with its
/// directory from the top of the repository; the top project is at `prefix`, which is empty
/// or ends with `/`.
pub(super) fn located<'a>(checked: &'a Checked, prefix: &str) -> Vec<Located<'a>> {
    let top = Located { directory: prefix.to_string(), report: &checked.top };
    let members = checked
        .members
        .iter()
        .map(|m| Located { directory: format!("{prefix}{}", m.path), report: &m.report });
    std::iter::once(top).chain(members).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Temp, git};

    #[test]
    fn a_page_is_named_from_the_top_of_the_repository() {
        let at = |directory: &str, page: &str| {
            let report = super::super::tests::report(vec![], None);
            Located { directory: directory.into(), report: &report }.path(page)
        };
        assert_eq!(at("", "docs/a.md"), "docs/a.md");
        assert_eq!(at("site/", "docs/a.md"), "site/docs/a.md");
        assert_eq!(at("site//./packages/acme/", "docs/a.md"), "site/packages/acme/docs/a.md");
        assert_eq!(at("site/tools/../packages/acme", "./docs/a.md"), "site/packages/acme/docs/a.md");
    }

    #[test]
    fn the_prefix_is_gits_and_a_project_outside_a_repository_is_refused() {
        let repository = Temp::new("report-prefix");
        repository.write("site/docattest.config.json", "{}");
        git(&repository.0, &["init", "--quiet"]);
        assert_eq!(prefix(&repository.0.join("site")).unwrap(), "site/");
        assert_eq!(prefix(&repository.0).unwrap(), "");
        let outside = Temp::new("report-outside");
        let refused = prefix(&outside.0).unwrap_err().to_string();
        assert!(refused.ends_with("is not in one"), "{refused}");
    }
}
