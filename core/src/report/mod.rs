//! Results, coverage, ordering and the text and JSON reports (`src/report/`).

mod attention;
pub mod members;
pub mod order;
pub mod outcome;

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::counted::counted;
use crate::text::compare_utf16;
pub use outcome::{Outcome, Status};

/// Page coverage: which living pages carry attestations.
#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    pub living: usize,
    pub covered: usize,
    pub uncovered: Vec<String>,
    pub frozen: Vec<String>,
    pub attestations: usize,
    pub weak: usize,
    pub cited: usize,
}

/// One surface in the report: symbol coverage and resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceCoverage {
    pub symbols: usize,
    pub attested: usize,
    pub unattested: Vec<String>,
    /// As the adapter described it; absent when the surface could not be resolved.
    pub resolution: Option<Value>,
    pub warnings: Vec<String>,
}

/// The age of one snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotStatus {
    pub name: String,
    pub source: String,
    pub fetched_at: String,
    pub age_days: f64,
    pub warning: bool,
}

/// How much the text report says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// Every result, then coverage, the surfaces, the snapshots and the changes.
    Full,
    /// As [`Detail::Full`], with the project's coverage as counts only: the check ran the
    /// pages it was given, and which other pages are uncovered is not what was asked.
    Pages,
    /// Only what someone has to act on (`--quiet`).
    Attention,
}

/// Everything `check` found.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub results: Vec<Outcome>,
    pub coverage: Coverage,
    pub surfaces: Vec<(String, SurfaceCoverage)>,
    pub snapshots: Vec<SnapshotStatus>,
    /// What the config names that does nothing in the project; never changes the exit code.
    pub warnings: Vec<String>,
    pub exit_code: i32,
    /// What the working tree did to the attestations of the base. Only with `--base`.
    pub changes: Option<Vec<crate::changes::Change>>,
}

/// Results by page, then line; ties keep their planned order, so reports are stable.
pub fn order_results(mut results: Vec<Outcome>) -> Vec<Outcome> {
    results.sort_by(|a, b| compare_utf16(&a.page, &b.page).then(a.line.cmp(&b.line)));
    results
}

/// 1 when any result is `fail` or `error`; skips, warnings and uncovered pages never count.
pub fn exit_code_for(results: &[Outcome]) -> i32 {
    i32::from(results.iter().any(Outcome::is_failure))
}

/// Page coverage from the number of results each living page produced.
pub fn coverage_for(
    living: &[String],
    frozen: &[String],
    counts: &HashMap<String, usize>,
    weak: usize,
    cited: usize,
) -> Coverage {
    let mut uncovered: Vec<String> =
        living.iter().filter(|p| counts.get(*p).copied().unwrap_or(0) == 0).cloned().collect();
    uncovered.sort_by(|a, b| compare_utf16(a, b));
    let mut frozen = frozen.to_vec();
    frozen.sort_by(|a, b| compare_utf16(a, b));
    Coverage {
        living: living.len(),
        covered: living.len() - uncovered.len(),
        uncovered,
        frozen,
        attestations: counts.values().sum(),
        weak,
        cited,
    }
}

/// A number as JSON, a whole one (a snapshot's age in days) as an integer: `90`, not `90.0`.
fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 { json!(n as i64) } else { json!(n) }
}

impl Report {
    /// The report as `--json` prints it, keys in the order the recordings hold.
    pub fn to_json(&self) -> Value {
        let c = &self.coverage;
        let mut surfaces = Map::new();
        for (name, s) in &self.surfaces {
            let mut m = Map::new();
            m.insert("symbols".into(), json!(s.symbols));
            m.insert("attested".into(), json!(s.attested));
            m.insert("unattested".into(), json!(s.unattested));
            if let Some(resolution) = &s.resolution {
                m.insert("resolution".into(), resolution.clone());
            }
            m.insert("warnings".into(), json!(s.warnings));
            surfaces.insert(name.clone(), Value::Object(m));
        }
        let snapshots: Vec<Value> = self
            .snapshots
            .iter()
            .map(|s| {
                json!({ "name": s.name, "source": s.source, "fetchedAt": s.fetched_at,
                        "ageDays": number(s.age_days), "warning": s.warning })
            })
            .collect();
        let mut report = json!({
            "results": self.results.iter().map(Outcome::to_json).collect::<Vec<_>>(),
            "coverage": {
                "living": c.living, "covered": c.covered, "uncovered": c.uncovered,
                "frozen": c.frozen, "attestations": c.attestations, "weak": c.weak, "cited": c.cited,
            },
            "surfaces": surfaces,
            "snapshots": snapshots,
            "exitCode": self.exit_code,
        });
        let Some(object) = report.as_object_mut() else { return report };
        if !self.warnings.is_empty() {
            object.insert("warnings".into(), json!(self.warnings));
        }
        if let Some(changes) = &self.changes {
            object.insert("changes".into(), changes.iter().map(crate::changes::Change::to_json).collect());
        }
        report
    }

    /// The report as `docattest check` prints it, ending with the verdict.
    pub fn format(&self, detail: Detail) -> String {
        let mut lines = match detail {
            Detail::Full | Detail::Pages => self.full_lines(detail),
            Detail::Attention => self.attention_lines(),
        };
        lines.push(verdict(self.exit_code).into());
        lines.join("\n")
    }

    /// Every finding by page, then the summary lines.
    fn full_lines(&self, detail: Detail) -> Vec<String> {
        let mut lines = self.finding_lines(|_| true);
        let count = |s: Status| self.results.iter().filter(|r| r.status == s).count();
        lines.push(format!(
            "results: {} pass, {} fail, {} error, {} skipped",
            count(Status::Pass),
            count(Status::Fail),
            count(Status::Error),
            count(Status::Skipped)
        ));
        lines.extend(self.coverage_lines(detail));
        lines.extend(self.warning_lines());
        lines.extend(self.surface_lines());
        lines.push(self.snapshots_line());
        lines.extend(crate::changes::change_lines(self.changes.as_deref()));
        lines
    }

    /// Each finding that is `shown`, under its page.
    fn finding_lines(&self, shown: impl Fn(&Outcome) -> bool) -> Vec<String> {
        let mut lines = Vec::new();
        let mut page: Option<&str> = None;
        for r in self.results.iter().filter(|r| shown(r)) {
            if page != Some(r.page.as_str()) {
                page = Some(&r.page);
                lines.push(r.page.clone());
            }
            let mark = status_mark(r.status);
            lines.push(format!("  {mark} L{} {}: {}", r.line, r.kind.as_str(), r.message));
        }
        lines
    }

    /// The coverage line, then the uncovered pages by name unless only some pages were run.
    /// Frozen pages are counted, never named: freezing them was the config's decision.
    fn coverage_lines(&self, detail: Detail) -> Vec<String> {
        let c = &self.coverage;
        let cited = if c.cited > 0 { format!(", {} cited", c.cited) } else { String::new() };
        let mut lines = vec![format!(
            "coverage: {}, {} covered ({}, {} weak{cited}), {} uncovered, {} frozen",
            counted(c.living, "living page"),
            c.covered,
            counted(c.attestations, "attestation"),
            c.weak,
            c.uncovered.len(),
            c.frozen.len()
        )];
        if detail == Detail::Full && !c.uncovered.is_empty() {
            lines.extend(wrapped("  uncovered: ", &c.uncovered));
        }
        lines
    }

    /// One line for each of the config's warnings.
    fn warning_lines(&self) -> Vec<String> {
        self.warnings.iter().map(|warning| format!("warning: {warning}")).collect()
    }

    fn surface_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, s) in &self.surfaces {
            lines.push(format!(
                "surface {name}: {}, {} attested, {} not attested by any page",
                counted(s.symbols, "symbol"),
                s.attested,
                s.unattested.len()
            ));
            if let Some(r) = &s.resolution {
                lines.push(resolution_line(r));
            }
            for warning in &s.warnings {
                lines.push(format!("  warning: {warning}"));
            }
        }
        lines
    }

    fn snapshots_line(&self) -> String {
        if self.snapshots.is_empty() {
            return "snapshots: none".into();
        }
        let ages: Vec<String> = self.snapshots.iter().map(snapshot_age).collect();
        format!("snapshots: {}", ages.join(", "))
    }
}

/// How wide a line of the text report grows before a list goes on to the next line.
const TEXT_WIDTH: usize = 100;

/// `label` then `names` separated by commas, as many to a line as fit in [`TEXT_WIDTH`];
/// each line after the first is indented to where the names start. A name is never cut.
fn wrapped(label: &str, names: &[String]) -> Vec<String> {
    let margin = label.chars().count();
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for (k, name) in names.iter().enumerate() {
        let item = if k + 1 < names.len() { format!("{name},") } else { name.clone() };
        if !line.is_empty() && margin + line.chars().count() + 1 + item.chars().count() > TEXT_WIDTH {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&item);
    }
    lines.push(line);
    let indent = " ".repeat(margin);
    let starts = std::iter::once(label).chain(std::iter::repeat(indent.as_str()));
    starts.zip(lines).map(|(start, line)| format!("{start}{line}")).collect()
}

/// The last line of a report: `ok` when the exit code is 0, `FAILED` otherwise.
fn verdict(exit_code: i32) -> &'static str {
    if exit_code == 0 { "ok" } else { "FAILED" }
}

/// `<name> (<age> days)`, with `, stale` inside the brackets for one that is.
fn snapshot_age(s: &SnapshotStatus) -> String {
    let stale = if s.warning { ", stale" } else { "" };
    format!("{} ({}{stale})", s.name, counted(s.age_days, "day"))
}

/// The mark a finding of this status is printed with.
fn status_mark(status: Status) -> &'static str {
    match status {
        Status::Pass => "✓",
        Status::Fail => "✗",
        Status::Error => "!",
        Status::Skipped => "-",
    }
}

/// `  <import> → <entry> (<package>)`: what examples import, the file it is read from, and
/// the package that file belongs to, displayed as the adapter's language writes one
/// (`package.display`). The shape was checked when the description was read.
fn resolution_line(r: &Value) -> String {
    let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_string();
    let package = r.get("package").map(|p| format!(" ({})", text(p.get("display")))).unwrap_or_default();
    format!("  {} → {}{package}", text(r.get("import")), text(r.get("entry")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(n: usize) -> Vec<String> {
        (1..=n).map(|k| format!("docs/guide/page-{k:02}.md")).collect()
    }

    fn report(uncovered: Vec<String>, frozen: Vec<String>) -> Report {
        let living = uncovered.len() + 1;
        let coverage = Coverage { living, covered: 1, uncovered, frozen, attestations: 1, weak: 0, cited: 0 };
        let (surfaces, snapshots, warnings) = (Vec::new(), Vec::new(), Vec::new());
        Report { results: Vec::new(), coverage, surfaces, snapshots, warnings, exit_code: 0, changes: None }
    }

    #[test]
    fn a_long_list_goes_on_over_lines_that_fit_and_a_short_one_stays_on_one() {
        assert_eq!(
            wrapped("  uncovered: ", &pages(2)),
            ["  uncovered: docs/guide/page-01.md, docs/guide/page-02.md"]
        );
        let lines = wrapped("  uncovered: ", &pages(9));
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| line.chars().count() <= TEXT_WIDTH), "{lines:#?}");
        assert!(lines[1].starts_with("             docs/guide/page-04.md,"), "{lines:#?}");
        assert!(lines[2].ends_with("docs/guide/page-09.md"), "{lines:#?}");
        assert_eq!(
            lines.join(" ").split_whitespace().skip(1).collect::<Vec<_>>().join(" "),
            pages(9).join(", ")
        );
    }

    #[test]
    fn frozen_pages_are_counted_and_uncovered_ones_named_only_when_every_page_ran() {
        let both = report(pages(1), vec!["docs/old.md".into()]);
        let full = both.format(Detail::Full);
        assert!(full.contains("1 uncovered, 1 frozen"), "{full}");
        assert!(full.contains("\n  uncovered: docs/guide/page-01.md\n"), "{full}");
        assert!(!full.contains("docs/old.md"), "{full}");
        let named = both.format(Detail::Pages);
        assert!(!named.contains("page-01") && named.contains("1 uncovered, 1 frozen"), "{named}");
        assert_eq!(both.to_json()["coverage"]["frozen"], json!(["docs/old.md"]));
    }

    #[test]
    fn the_configs_warnings_are_printed_and_kept_in_the_json_only_when_there_are_some() {
        let mut warned = report(Vec::new(), Vec::new());
        assert!(warned.to_json().get("warnings").is_none());
        warned.warnings.push("\"pages\" has \"guide/*.md\", which matches no file".into());
        assert!(
            warned
                .format(Detail::Full)
                .contains("\nwarning: \"pages\" has \"guide/*.md\", which matches no file\n")
        );
        assert_eq!(
            warned.to_json()["warnings"],
            json!(["\"pages\" has \"guide/*.md\", which matches no file"])
        );
        assert_eq!(warned.exit_code, 0);
    }
}
