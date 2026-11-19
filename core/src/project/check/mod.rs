//! Running a check through the adapter, and the parsing-only queries.

mod checker;
mod cited_runs;
mod display;
mod exports_split;
mod problems;
mod sample;

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};

use super::config_warnings::undocumented_warnings;
use super::state::State;
use crate::adapter::Pending;
use crate::api::attestation::{Attestation, Body, Kind};
use crate::api::errors::{Error, Result};
use crate::report::order::by_page_and_line;
use crate::report::{
    Coverage, Outcome, PageSets, Report, SurfaceCoverage, coverage_for, exit_code_for, order_results,
};
use crate::text::compare_utf16;
use checker::Checker;
pub use cited_runs::CitedTests;
use cited_runs::refuse_without_runner;
use exports_split::split_exports;
use problems::{Problems, send_problems};

/// A surface symbol an attestation exercises; `surface` is None when none exports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRef {
    pub name: String,
    pub surface: Option<String>,
}

/// Symbols of every attestation, keyed by id.
pub type Symbols = HashMap<String, Vec<SymbolRef>>;

/// One place a symbol is attested, with the sentence the proof defends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub page: String,
    pub line: usize,
    pub kind: Kind,
    pub claim: String,
}

impl Reference {
    pub fn to_json(&self) -> Value {
        json!({ "page": self.page, "line": self.line, "kind": self.kind.as_str(), "claim": self.claim })
    }
}

/// Whether examples tagged `network` may reach the network in this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Allowed,
    Off,
}

/// Open the session: the adapter context for this project; `examples` are the ones the
/// session will compile. Requests may follow straight away.
pub(crate) fn open(state: &State, network: Network, examples: &[&Attestation]) -> Pending {
    let mut data = Map::new();
    for s in &state.snapshots.by_name {
        data.insert(s.name.clone(), s.data.clone());
    }
    let mut snapshots = Map::new();
    if let Some(dir) = &state.config.snapshots_dir {
        snapshots.insert("dir".into(), json!(dir));
    }
    snapshots.insert("data".into(), Value::Object(data));
    let mut params = Map::new();
    params.insert("root".into(), json!(state.files.root().to_string_lossy()));
    // The surfaces as the config states them: what an entry means is the adapter's to say.
    let surfaces =
        state.config.surfaces.iter().map(|(name, entry)| json!({ "name": name, "entry": entry.to_json() }));
    params.insert("surfaces".into(), surfaces.collect());
    if let Some(setup) = &state.config.setup {
        params.insert("setup".into(), json!(setup));
    }
    params.insert("timeoutMs".into(), json!(state.config.timeout_ms));
    params.insert("workers".into(), json!(state.config.workers));
    params.insert("network".into(), json!(network == Network::Allowed));
    params.insert("snapshots".into(), Value::Object(snapshots));
    params.extend(state.fact_files.session_params());
    params.insert("examples".into(), examples.iter().map(|a| a.to_json()).collect());
    state.adapter.send("open", Value::Object(params))
}

fn symbols_request(state: &State) -> Pending {
    let all: Vec<Value> =
        state.attestations.iter().filter(|a| !a.is_run_by_core()).map(Attestation::to_json).collect();
    state.adapter.send("symbols", json!({ "attestations": all }))
}

fn parse_symbols(value: Value) -> Result<Symbols> {
    let Value::Object(map) = value else {
        return Err(Error::internal("symbols did not answer an object"));
    };
    map.into_iter().map(|(id, refs)| Ok((id, symbol_refs(&refs)?))).collect()
}

/// The symbols one attestation exercises, as `symbols` answers them.
fn symbol_refs(refs: &Value) -> Result<Vec<SymbolRef>> {
    let refs =
        refs.as_array().ok_or_else(|| Error::internal("symbols answered something that is not a list"))?;
    refs.iter()
        .map(|r| {
            let name = r.get("name").and_then(Value::as_str);
            let name = name.ok_or_else(|| Error::internal("symbols answered a symbol without a name"))?;
            Ok(SymbolRef {
                name: name.to_string(),
                surface: r.get("surface").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect()
}

/// Unique symbol names, in first-seen order.
fn symbol_names(refs: &[SymbolRef]) -> Vec<String> {
    let mut seen = HashSet::new();
    refs.iter().filter(|r| seen.insert(r.name.as_str())).map(|r| r.name.clone()).collect()
}

/// Names each surface's attestations reference, keyed by surface.
fn attested_by_surface(symbols: &Symbols) -> HashMap<&str, HashSet<&str>> {
    let mut out: HashMap<&str, HashSet<&str>> = HashMap::new();
    for refs in symbols.values() {
        for r in refs {
            if let Some(surface) = &r.surface {
                out.entry(surface.as_str()).or_default().insert(r.name.as_str());
            }
        }
    }
    out
}

/// The answer to `exports`: the names, or `None` when the surface cannot be resolved.
fn export_names(value: Value) -> Option<Vec<String>> {
    value.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

/// Run the attestations on the selected pages; coverage still covers every living page.
pub fn run_check(
    state: &State,
    network: Network,
    selected: &HashSet<String>,
    cited: CitedTests,
) -> Result<Report> {
    refuse_without_runner(&state.adapter, cited)?;
    let chosen: Vec<&Attestation> =
        state.attestations.iter().filter(|a| selected.contains(&a.page)).collect();
    let examples: Vec<&Attestation> =
        chosen.iter().copied().filter(|a| matches!(a.body, Body::Example(_)) && a.ok()).collect();
    let opened = open(state, network, &examples);
    // Facts, citations and tables depend on nothing else: they go first, so the adapter's
    // workers run them while it builds its compiler program for symbols and compile. Answers
    // are still awaited in a fixed order (the session, symbols, then the checks), so when
    // several requests fail, the same one is reported every time.
    let mut checker = Checker::new(state, network, cited);
    let started = checker.start(&chosen);
    let symbols_answer = symbols_request(state);
    let compiled = checker.send_compile(&examples);
    let described = state.adapter.send("describeSurfaces", json!({}));
    let problems = send_problems(&state.adapter);
    let exported: Vec<Pending> = state
        .config
        .surfaces
        .iter()
        .map(|(name, _)| state.adapter.send("exports", json!({ "surface": name })))
        .collect();
    opened.wait()?;
    checker.symbols = parse_symbols(symbols_answer.wait()?)?;
    let checked = checker.check(&chosen, started, compiled)?;
    let mut all = state.snapshots.errors.clone();
    all.extend(checked.into_iter().flatten());
    let mut coverage = coverage_of(state, &all, selected);
    coverage.cited_run = (cited == CitedTests::Run).then_some(checker.cited_ran);
    let SurfaceReports { surfaces, warnings } =
        surface_reports(state, &checker.symbols, described, exported)?;
    // A problem fails the check but attests nothing, so it is added after coverage is counted.
    let problems = Problems::wait(problems)?;
    all.extend(problems.results);
    let results = order_results(all);
    let exit_code = exit_code_for(&results);
    Ok(Report {
        results,
        coverage,
        surfaces,
        snapshots: state.snapshots.statuses.clone(),
        warnings: [state.warnings.clone(), warnings, problems.warnings].concat(),
        exit_code,
        changes: None,
    })
}

/// Page coverage from the results of the attestations.
fn coverage_of(state: &State, results: &[Outcome], selected: &HashSet<String>) -> Coverage {
    let pages = PageSets { living: &state.living, prose: &state.prose, frozen: &state.frozen };
    coverage_for(
        pages,
        &result_counts(state, results, selected),
        state.attestations.iter().filter(|a| matches!(&a.body, Body::Example(e) if e.weak) && a.ok()).count(),
        // A broken citation is still a citation: its error is counted among the attestations,
        // and must not be counted as a proof.
        state.attestations.iter().filter(|a| a.kind() == Kind::Test).count(),
    )
}

// Pages that were not run count what they would produce, so coverage never shrinks when
// only some pages are checked.
fn result_counts(state: &State, results: &[Outcome], selected: &HashSet<String>) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for r in results {
        *counts.entry(r.page.clone()).or_default() += 1;
    }
    for a in &state.attestations {
        if !selected.contains(&a.page) {
            let n = match &a.body {
                Body::Cases(c) if a.ok() => c.rows.len(),
                _ => 1,
            };
            *counts.entry(a.page.clone()).or_default() += n;
        }
    }
    counts
}

/// Each surface's coverage, and the config's warnings that only its exports can tell.
struct SurfaceReports {
    surfaces: Vec<(String, SurfaceCoverage)>,
    warnings: Vec<String>,
}

fn surface_reports(
    state: &State,
    symbols: &Symbols,
    described: Pending,
    exported: Vec<Pending>,
) -> Result<SurfaceReports> {
    let described = described.wait()?;
    let attested = attested_by_surface(symbols);
    let mut reports = SurfaceReports { surfaces: Vec::new(), warnings: Vec::new() };
    for ((name, _), exports) in state.config.surfaces.iter().zip(exported) {
        let names = export_names(exports.wait()?);
        if let Some(names) = &names {
            reports.warnings.extend(undocumented_warnings(&state.config, name, names)?);
        }
        let globs = state.config.undocumented_globs(name);
        let split = split_exports(names.unwrap_or_default(), attested.get(name.as_str()), globs)?;
        let (resolution, warnings) = description_of(name, described.get(name))?;
        let coverage = SurfaceCoverage {
            symbols: split.attested + split.unattested.len() + split.undocumented.len(),
            attested: split.attested,
            unattested: split.unattested,
            undocumented: (!globs.is_empty()).then_some(split.undocumented),
            resolution,
            warnings,
        };
        reports.surfaces.push((name.clone(), coverage));
    }
    Ok(reports)
}

/// The resolution and the warnings `describeSurfaces` gave a surface.
fn description_of(surface: &str, description: Option<&Value>) -> Result<(Option<Value>, Vec<String>)> {
    let resolution = description.and_then(|d| d.get("resolution")).filter(|r| truthy(r)).cloned();
    if let Some(resolution) = &resolution {
        resolved(surface, resolution)?;
    }
    let warnings = description
        .and_then(|d| d.get("warnings"))
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|w| w.as_str().map_or_else(|| w.to_string(), str::to_string)).collect())
        .unwrap_or_default();
    Ok((resolution, warnings))
}

/// A resolution names what is imported and the entry it is read from; a package, when it
/// names one, says how it is displayed. The report prints these, so they must be there.
fn resolved(surface: &str, resolution: &Value) -> Result<()> {
    let string = |v: Option<&Value>| v.is_some_and(Value::is_string);
    let package = resolution.get("package");
    let shown = package.is_none_or(|p| string(p.get("name")) && string(p.get("display")));
    if string(resolution.get("import")) && string(resolution.get("entry")) && shown {
        return Ok(());
    }
    Err(Error::internal(format!(
        "describeSurfaces answered a resolution for \"{surface}\" without an import, an entry or a package's name and display"
    )))
}

fn truthy(v: &Value) -> bool {
    !matches!(v, Value::Null | Value::Bool(false)) && v.as_str() != Some("")
}

/// The results of `chosen`, each attestation's in a list of its own, in the order given, as
/// `check` would give them.
pub fn results_of(state: &State, network: Network, chosen: &[&Attestation]) -> Result<Vec<Vec<Outcome>>> {
    results_and_symbols(state, network, chosen).map(|(results, _)| results)
}

/// [`results_of`], and the symbols of every attestation of the project. The examples among
/// `chosen` are the session's, and are compiled before they run; citations are judged from
/// their test files, as `check` without `--run-cited` judges them. The session stays open
/// for more requests.
pub(crate) fn results_and_symbols(
    state: &State,
    network: Network,
    chosen: &[&Attestation],
) -> Result<(Vec<Vec<Outcome>>, Symbols)> {
    let examples: Vec<&Attestation> =
        chosen.iter().copied().filter(|a| matches!(a.body, Body::Example(_)) && a.ok()).collect();
    let opened = open(state, network, &examples);
    let mut checker = Checker::new(state, network, CitedTests::Declared);
    let started = checker.start(chosen);
    let symbols = symbols_request(state);
    let compiled = checker.send_compile(&examples);
    opened.wait()?;
    checker.symbols = parse_symbols(symbols.wait()?)?;
    let results = checker.check(chosen, started, compiled)?;
    Ok((results, checker.symbols))
}

/// Queries parse and resolve symbols but never run anything, so the session compiles nothing.
pub fn symbols_only(state: &State, network: Network) -> Result<Symbols> {
    let opened = open(state, network, &[]);
    let symbols = symbols_request(state);
    opened.wait()?;
    parse_symbols(symbols.wait()?)
}

/// Open a session that compiles nothing; requests may follow straight away.
pub(crate) fn open_session(state: &State, network: Network) -> Pending {
    open(state, network, &[])
}

/// The symbols of every attestation, asked of an open session.
pub(crate) fn request_symbols(state: &State) -> Result<Symbols> {
    parse_symbols(symbols_request(state).wait()?)
}

/// The symbol → references map, keys sorted, references by page then line.
pub fn build_index(attestations: &[Attestation], symbols: &Symbols) -> Vec<(String, Vec<Reference>)> {
    let mut map: HashMap<String, Vec<Reference>> = HashMap::new();
    for a in attestations {
        let refs = symbols.get(&a.id).map_or(&[][..], Vec::as_slice);
        for name in symbol_names(refs) {
            map.entry(name).or_default().push(Reference {
                page: a.page.clone(),
                line: a.line,
                kind: a.kind(),
                claim: a.claim.clone(),
            });
        }
    }
    let mut index: Vec<(String, Vec<Reference>)> = map.into_iter().collect();
    index.sort_by(|a, b| compare_utf16(&a.0, &b.0));
    for (_, refs) in &mut index {
        refs.sort_by(|a, b| by_page_and_line((&a.page, a.line), (&b.page, b.line)));
    }
    index
}

/// Exports of a surface that no attestation against it references, less those the config
/// says are not meant to be documented.
pub fn uncovered(state: &State, network: Network, surface: &str) -> Result<Vec<String>> {
    if !state.config.surfaces.iter().any(|(name, _)| name == surface) {
        let configured: Vec<&str> = state.config.surfaces.iter().map(|(name, _)| name.as_str()).collect();
        return Err(Error::user(format!(
            "unknown surface \"{surface}\" (configured: {})",
            configured.join(", ")
        )));
    }
    let opened = open(state, network, &[]);
    let symbols = symbols_request(state);
    let exported = state.adapter.send("exports", json!({ "surface": surface }));
    opened.wait()?;
    let symbols = parse_symbols(symbols.wait()?)?;
    let names = export_names(exported.wait()?).unwrap_or_default();
    let attested = attested_by_surface(&symbols);
    let split = split_exports(names, attested.get(surface), state.config.undocumented_globs(surface))?;
    Ok(split.unattested)
}
