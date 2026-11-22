//! Per-surface coverage and resolution reporting for `check`.

use std::collections::HashSet;

use serde_json::Value;

use super::{Symbols, attested_by_surface, export_names};
use crate::adapter::Pending;
use crate::api::errors::{Error, Result};
use crate::project::check::exports_split::split_exports;
use crate::project::config_warnings::undocumented_warnings;
use crate::project::state::State;
use crate::report::SurfaceCoverage;

/// Each surface's coverage, the config's warnings that only its exports can tell, and the
/// union of all resolved surfaces' export names.
pub(super) struct SurfaceReports {
    pub(super) surfaces: Vec<(String, SurfaceCoverage)>,
    pub(super) warnings: Vec<String>,
    pub(super) exports: Vec<String>,
}

pub(super) fn surface_reports(
    state: &State,
    symbols: &Symbols,
    described: Pending,
    exported: Vec<Pending>,
) -> Result<SurfaceReports> {
    let described = described.wait()?;
    let attested = attested_by_surface(symbols);
    let mut reports = SurfaceReports { surfaces: Vec::new(), warnings: Vec::new(), exports: Vec::new() };
    let mut seen_exports: HashSet<String> = HashSet::new();
    for ((name, entry), exports) in state.config.surfaces.iter().zip(exported) {
        let names = export_names(exports.wait()?);
        let (resolution, warnings) = description_of(name, described.get(name))?;
        record_exports(&mut reports.exports, &mut seen_exports, names.as_deref());
        if entry.is_internal() {
            reports.warnings.extend(warnings.into_iter().map(|w| format!("surface {name}: {w}")));
            continue;
        }
        let coverage =
            public_coverage(state, name, names, attested.get(name.as_str()), &mut reports.warnings)?;
        reports.surfaces.push((name.clone(), SurfaceCoverage { resolution, warnings, ..coverage }));
    }
    Ok(reports)
}

fn record_exports(exports: &mut Vec<String>, seen: &mut HashSet<String>, names: Option<&[String]>) {
    for export in names.into_iter().flatten() {
        if seen.insert(export.clone()) {
            exports.push(export.clone());
        }
    }
}

fn public_coverage(
    state: &State,
    name: &str,
    names: Option<Vec<String>>,
    attested: Option<&HashSet<&str>>,
    config_warnings: &mut Vec<String>,
) -> Result<SurfaceCoverage> {
    if let Some(names) = &names {
        config_warnings.extend(undocumented_warnings(&state.config, name, names)?);
    }
    let globs = state.config.undocumented_globs(name);
    let split = split_exports(names.unwrap_or_default(), attested, globs)?;
    Ok(SurfaceCoverage {
        symbols: split.attested + split.unattested.len() + split.undocumented.len(),
        attested: split.attested,
        unattested: split.unattested,
        undocumented: (!globs.is_empty()).then_some(split.undocumented),
        resolution: None,
        warnings: Vec::new(),
    })
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
