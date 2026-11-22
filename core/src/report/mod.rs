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
    /// Living pages marked as having nothing to prove, and carrying no attestation.
    pub prose: Vec<String>,
    pub frozen: Vec<String>,
    /// The results the living pages produce: one for each attestation, one for each row of a
    /// cases table.
    pub attestations: usize,
    /// `check` examples that assert nothing: proofs, never citations.
    pub weak: usize,
    /// `test` citations, broken ones too: each names a test, and proves nothing itself.
    pub cited: usize,
    /// With `check --run-cited`, the citations whose test ran, whatever it gave; None when
    /// the check did not run cited tests.
    pub cited_run: Option<usize>,
}

impl Coverage {
    /// The attestations that are not citations: what docattest itself runs or evaluates.
    pub fn proofs(&self) -> usize {
        self.attestations - self.cited
    }
}

/// One surface in the report: symbol coverage and resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceCoverage {
    pub symbols: usize,
    pub attested: usize,
    /// Neither attested nor matched by an `undocumented` glob.
    pub unattested: Vec<String>,
    /// Not attested, and matched by an `undocumented` glob; `None` when the config gives the
    /// surface no such glob.
    pub undocumented: Option<Vec<String>>,
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

/// A project's pages by how they stand.
pub struct PageSets<'a> {
    pub living: &'a [String],
    /// Living pages with nothing to prove.
    pub prose: &'a [String],
    pub frozen: &'a [String],
}

/// Page coverage from the number of results each page produced. Results on other files (a
/// broken snapshot file) are not attestations, and are left out of the count. A prose page
/// with no result is counted apart, never as uncovered.
pub fn coverage_for(pages: PageSets, counts: &HashMap<String, usize>, weak: usize, cited: usize) -> Coverage {
    let unproven = |p: &&String| counts.get(*p).copied().unwrap_or(0) == 0;
    let sorted = |mut pages: Vec<String>| {
        pages.sort_by(|a, b| compare_utf16(a, b));
        pages
    };
    let (prose, uncovered): (Vec<String>, Vec<String>) =
        pages.living.iter().filter(unproven).cloned().partition(|p| pages.prose.contains(p));
    let (prose, uncovered) = (sorted(prose), sorted(uncovered));
    Coverage {
        living: pages.living.len(),
        covered: pages.living.len() - uncovered.len() - prose.len(),
        uncovered,
        prose,
        frozen: sorted(pages.frozen.to_vec()),
        attestations: pages.living.iter().filter_map(|p| counts.get(p)).sum(),
        weak,
        cited,
        cited_run: None,
    }
}

/// Coverage as `--json` prints it. Prose pages are listed only in a project that has some,
/// so a project that marks none reports what it always did.
fn coverage_json(c: &Coverage) -> Value {
    let mut m = Map::new();
    m.insert("living".into(), json!(c.living));
    m.insert("covered".into(), json!(c.covered));
    m.insert("uncovered".into(), json!(c.uncovered));
    if !c.prose.is_empty() {
        m.insert("prose".into(), json!(c.prose));
    }
    m.insert("frozen".into(), json!(c.frozen));
    m.insert("attestations".into(), json!(c.attestations));
    m.insert("proofs".into(), json!(c.proofs()));
    m.insert("weak".into(), json!(c.weak));
    m.insert("cited".into(), json!(c.cited));
    if let Some(run) = c.cited_run {
        m.insert("citedRun".into(), json!(run));
    }
    Value::Object(m)
}

/// A number as JSON, a whole one (a snapshot's age in days) as an integer: `90`, not `90.0`.
fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 { json!(n as i64) } else { json!(n) }
}

impl Report {
    /// The report as `--json` prints it, keys in the order the recordings hold.
    pub fn to_json(&self) -> Value {
        let mut surfaces = Map::new();
        for (name, s) in &self.surfaces {
            let mut m = Map::new();
            m.insert("symbols".into(), json!(s.symbols));
            m.insert("attested".into(), json!(s.attested));
            m.insert("unattested".into(), json!(s.unattested));
            if let Some(undocumented) = &s.undocumented {
                m.insert("undocumented".into(), json!(undocumented));
            }
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
            "coverage": coverage_json(&self.coverage),
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
        lines.extend(self.failure_recap_lines());
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
    /// Proofs and citations are always counted apart, and summed: a citation only says where
    /// to look, so a total alone would make coverage look stronger than it is. Prose and
    /// frozen pages are counted, never named: marking them was the project's decision. Prose
    /// is counted only in a project that has some. Citations whose test ran are counted among
    /// the citations, not the proofs: a run shows that the test passes, not that it tests the
    /// sentence.
    fn coverage_lines(&self, detail: Detail) -> Vec<String> {
        let c = &self.coverage;
        let prose = if c.prose.is_empty() { String::new() } else { format!(", {} prose", c.prose.len()) };
        let run = c.cited_run.map_or_else(String::new, |run| format!(", {run} of them run"));
        let mut lines = vec![format!(
            "coverage: {}, {} covered ({}: {}, {} of them weak, and {}{run}), {} uncovered{prose}, {} frozen",
            counted(c.living, "living page"),
            c.covered,
            counted(c.attestations, "attestation"),
            counted(c.proofs(), "proof"),
            c.weak,
            counted(c.cited, "citation"),
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
            let undocumented = s
                .undocumented
                .as_ref()
                .map_or_else(String::new, |names| format!(", {} not meant to be documented", names.len()));
            lines.push(format!(
                "surface {name}: {}, {} attested, {} not attested by any page{undocumented}",
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
pub(crate) fn status_mark(status: Status) -> &'static str {
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
mod tests;
