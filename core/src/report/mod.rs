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
        if let (Some(changes), Some(object)) = (&self.changes, report.as_object_mut()) {
            object.insert("changes".into(), changes.iter().map(crate::changes::Change::to_json).collect());
        }
        report
    }

    /// The report as `docattest check` prints it, ending with the verdict.
    pub fn format(&self, detail: Detail) -> String {
        let mut lines = match detail {
            Detail::Full => self.full_lines(),
            Detail::Attention => self.attention_lines(),
        };
        lines.push(verdict(self.exit_code).into());
        lines.join("\n")
    }

    /// Every finding by page, then the summary lines.
    fn full_lines(&self) -> Vec<String> {
        let mut lines = self.finding_lines(|_| true);
        let count = |s: Status| self.results.iter().filter(|r| r.status == s).count();
        lines.push(format!(
            "results: {} pass, {} fail, {} error, {} skipped",
            count(Status::Pass),
            count(Status::Fail),
            count(Status::Error),
            count(Status::Skipped)
        ));
        lines.extend(self.coverage_lines());
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

    fn coverage_lines(&self) -> Vec<String> {
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
        if !c.uncovered.is_empty() {
            lines.push(format!("  uncovered: {}", c.uncovered.join(", ")));
        }
        if !c.frozen.is_empty() {
            lines.push(format!("  frozen: {}", c.frozen.join(", ")));
        }
        lines
    }

    fn surface_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, s) in &self.surfaces {
            lines.push(format!(
                "surface {name}: {}, {} attested, {} not referenced by any page",
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
