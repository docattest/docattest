//! What `check --quiet` prints: only what someone has to act on, so a coding agent or a git
//! hook reads nothing about the proofs that hold.

use super::{Outcome, Report};
use crate::changes::change_lines;

impl Report {
    /// The results that fail or error, under their pages; the config's and the surfaces'
    /// warnings; the stale snapshots; and with a base, what the change did to the proofs.
    /// Empty when there is nothing to act on.
    pub fn attention_lines(&self) -> Vec<String> {
        let mut lines = self.finding_lines(Outcome::is_failure);
        lines.extend(self.warning_lines());
        lines.extend(self.surface_warning_lines());
        lines.extend(self.stale_snapshots_line());
        if let Some(changes) = self.changes.as_deref().filter(|changes| !changes.is_empty()) {
            lines.extend(change_lines(Some(changes)));
        }
        lines
    }

    /// One line per failing result (and failing base change), optionally prefixed with a
    /// member path. Shared by the single-project recap and the multi-project summary.
    pub(super) fn failure_entries(&self, prefix: Option<&str>) -> Vec<String> {
        let qualify = |page: &str| prefix.map_or_else(|| page.to_string(), |dir| format!("{dir}/{page}"));
        let mut out: Vec<String> = self
            .results
            .iter()
            .filter(|r| r.is_failure())
            .map(|r| format!("  {}:{} {}: {}", qualify(&r.page), r.line, r.kind.as_str(), r.message))
            .collect();
        if let Some(changes) = &self.changes {
            for c in changes.iter().filter(|c| c.severity == crate::changes::change::Severity::Fail) {
                out.push(format!("  {}:{} {}: {}", qualify(&c.page), c.line, c.kind.as_str(), c.message));
            }
        }
        out
    }

    /// When a full report mixes passing/skipped results with failures, repeat the failing
    /// lines right above `FAILED` so they do not get buried in long output.
    pub(super) fn failure_recap_lines(&self) -> Vec<String> {
        if !self.results.iter().any(|r| !r.is_failure()) {
            return Vec::new();
        }
        let entries: Vec<String> = self
            .results
            .iter()
            .filter(|r| r.is_failure())
            .map(|r| format!("  {}:{} {}: {}", r.page, r.line, r.kind.as_str(), r.message))
            .collect();
        if entries.is_empty() {
            return Vec::new();
        }
        std::iter::once("failures:".to_string()).chain(entries).collect()
    }

    fn surface_warning_lines(&self) -> Vec<String> {
        self.surfaces
            .iter()
            .flat_map(|(name, s)| {
                s.warnings.iter().map(move |warning| format!("surface {name}: warning: {warning}"))
            })
            .collect()
    }

    fn stale_snapshots_line(&self) -> Option<String> {
        let stale: Vec<String> =
            self.snapshots.iter().filter(|s| s.warning).map(super::snapshot_age).collect();
        (!stale.is_empty()).then(|| format!("snapshots: {}", stale.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::attestation::Kind;
    use crate::changes::Change;
    use crate::changes::change::{ChangeType, Severity};
    use crate::report::{Coverage, Detail, SnapshotStatus, Status, SurfaceCoverage};

    fn report(results: Vec<Outcome>) -> Report {
        let coverage = Coverage {
            living: 2,
            covered: 1,
            uncovered: vec!["docs/plain.md".into()],
            prose: vec![],
            frozen: vec!["docs/old.md".into()],
            attestations: results.len(),
            weak: 0,
            cited: 0,
            cited_run: None,
        };
        let exit_code = crate::report::exit_code_for(&results);
        let surface = SurfaceCoverage {
            symbols: 2,
            attested: 1,
            unattested: vec!["M".into()],
            undocumented: None,
            resolution: None,
            warnings: vec![],
        };
        let snapshot = SnapshotStatus {
            name: "rates".into(),
            source: "https://example.com/rates".into(),
            fetched_at: "2026-01-01".into(),
            age_days: 3.0,
            warning: false,
        };
        Report {
            results,
            coverage,
            surfaces: vec![("default".into(), surface)],
            snapshots: vec![snapshot],
            warnings: vec![],
            exit_code,
            changes: None,
        }
    }

    fn result(line: usize, status: Status) -> Outcome {
        Outcome::new("docs/a.md", line, Kind::Fact, status, &format!("{} at {line}", status.as_str()))
    }

    #[test]
    fn a_report_where_everything_holds_says_only_ok() {
        let holding = report(vec![result(1, Status::Pass), result(2, Status::Skipped)]);
        assert!(holding.attention_lines().is_empty());
        assert_eq!(holding.format(Detail::Attention), "ok");
    }

    #[test]
    fn only_failures_and_errors_are_listed_under_their_page() {
        let statuses = [Status::Pass, Status::Fail, Status::Skipped, Status::Error];
        let mixed = report(statuses.iter().enumerate().map(|(i, s)| result(i + 1, *s)).collect());
        assert_eq!(
            mixed.format(Detail::Attention),
            "docs/a.md\n  ✗ L2 fact: fail at 2\n  ! L4 fact: error at 4\nFAILED"
        );
    }

    #[test]
    fn warnings_and_stale_snapshots_need_attention_without_failing() {
        let mut warned = report(vec![result(1, Status::Pass)]);
        warned.warnings.push("\"pages\" has \"guide/*.md\", which matches no file".into());
        warned.surfaces[0].1.warnings.push("no declaration for M".into());
        warned.snapshots[0].warning = true;
        warned.snapshots[0].age_days = 90.0;
        assert_eq!(
            warned.format(Detail::Attention),
            "warning: \"pages\" has \"guide/*.md\", which matches no file\nsurface default: warning: no declaration for M\nsnapshots: rates (90 days, stale)\nok"
        );
    }

    #[test]
    fn changes_since_the_base_are_listed_only_when_there_are_some() {
        let mut unchanged = report(vec![]);
        unchanged.changes = Some(vec![]);
        assert_eq!(unchanged.format(Detail::Attention), "ok");

        let mut downgraded = report(vec![]);
        downgraded.changes = Some(vec![Change {
            kind: ChangeType::Downgrade,
            page: "docs/a.md".into(),
            line: 7,
            severity: Severity::Warning,
            message: "the example became `compile`".into(),
        }]);
        assert_eq!(
            downgraded.format(Detail::Attention),
            "changes since the base: 0 fail, 1 warning\n  ⚠ docs/a.md:7 downgrade: the example became `compile`\nok"
        );
    }

    #[test]
    fn a_full_report_recaps_failures_only_when_mixed_with_other_results() {
        let only_fail = report(vec![result(2, Status::Fail)]);
        assert!(!only_fail.format(Detail::Full).contains("\nfailures:\n"));
        let only_pass = report(vec![result(1, Status::Pass)]);
        assert!(!only_pass.format(Detail::Full).contains("\nfailures:\n"));
        let mixed = report(vec![result(1, Status::Pass), result(2, Status::Fail), result(4, Status::Error)]);
        let text = mixed.format(Detail::Full);
        assert!(
            text.ends_with(
                "\nfailures:\n  docs/a.md:2 fact: fail at 2\n  docs/a.md:4 fact: error at 4\nFAILED"
            ),
            "{text}"
        );
    }
}
