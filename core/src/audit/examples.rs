//! Auditing untagged code blocks that import a configured surface: compiling them through
//! the adapter surfaces broken examples as `drift` and valid ones as `match` (`compile`).

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::candidates::{ExampleCandidate, PageCandidates};
use super::{Status, Suggestion, Target};
use crate::adapter::Pending;
use crate::api::attestation::{Attestation, Body, Example, Mode};
use crate::api::errors::{Error, Result};
use crate::config::SurfaceEntry;
use crate::project::state::State;

/// A candidate untagged code block paired with the surface it imports and its synthetic attestation.
pub(super) struct PlannedExample {
    pub surface: String,
    pub import: String,
    pub attestation: Attestation,
}

/// Tags in a fence info string that signal an intentionally non-runnable or pseudocode snippet.
const SKIP_TAGS: [&str; 2] = ["ignore", "no_run"];

/// Collect untagged runnable code blocks that import a configured surface.
pub(super) fn plan_examples(state: &State, pages: &[(&String, PageCandidates)]) -> Vec<PlannedExample> {
    if state.adapter.block_languages.is_empty() {
        return Vec::new();
    }
    let imports = surface_imports(state);
    if imports.is_empty() {
        return Vec::new();
    }
    let mut planned = Vec::new();
    for (page, found) in pages {
        for candidate in &found.examples {
            if let Some(item) = plan_one(state, page, candidate, &imports, planned.len()) {
                planned.push(item);
            }
        }
    }
    planned
}

/// Ask the adapter to compile the planned untagged examples; returns None when there are none.
pub(super) fn send_compile(state: &State, planned: &[PlannedExample]) -> Option<Pending> {
    if planned.is_empty() {
        return None;
    }
    let list: Vec<Value> = planned.iter().map(|p| p.attestation.to_json()).collect();
    Some(state.adapter.send("compile", json!({ "examples": list })))
}

/// Turn the adapter's `compile` diagnostics into audit suggestions (`match` when compiling
/// cleanly, `drift` with the diagnostics when failing).
pub(super) fn collect_results(
    planned: &[PlannedExample],
    compiled: Option<Pending>,
) -> Result<Vec<Suggestion>> {
    let diagnostics = match compiled {
        Some(pending) => pending.wait()?.as_array().cloned().unwrap_or_default(),
        None => Vec::new(),
    };
    let mut out = Vec::with_capacity(planned.len());
    for item in planned {
        let errors = diagnostics_for(&diagnostics, &item.attestation.id)?;
        let status = if errors.is_empty() { Status::Match } else { Status::Drift };
        out.push(Suggestion {
            page: item.attestation.page.clone(),
            line: item.attestation.line,
            target: Target::Example,
            expression: item.import.clone(),
            status,
            missing: Vec::new(),
            unexpected: errors,
            surface: item.surface.clone(),
            directive: "compile".into(),
        });
    }
    Ok(out)
}

fn plan_one(
    state: &State,
    page: &str,
    candidate: &ExampleCandidate,
    imports: &[(String, String)],
    seq: usize,
) -> Option<PlannedExample> {
    let lang = runnable_lang(&candidate.info, &state.adapter.block_languages)?;
    if has_ellipsis_line(&candidate.code) {
        return None;
    }
    let (surface, import) = matched_surface(&candidate.code, imports)?;
    let attestation = Attestation {
        id: format!("__docattest_audit_example__:{seq}"),
        page: page.to_string(),
        line: candidate.line,
        claim: String::new(),
        body: Body::Example(Example {
            lang: lang.to_string(),
            mode: Mode::Compile,
            network: false,
            code: candidate.code.clone(),
            weak: false,
            owner: None,
            tags: Vec::new(),
        }),
        error: None,
    };
    Some(PlannedExample { surface: surface.to_string(), import: import.to_string(), attestation })
}

/// Surface import specifiers mapped to `(surface_name, import_specifier)`.
fn surface_imports(state: &State) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let root_pkg = root_package_name(state.files.root());
    for (name, entry) in &state.config.surfaces {
        if let Some(spec) = specifier_of(entry, root_pkg.as_deref())
            && seen.insert(spec.clone())
        {
            out.push((name.clone(), spec));
        }
    }
    out
}

fn specifier_of(entry: &SurfaceEntry, root_pkg: Option<&str>) -> Option<String> {
    if entry.is_internal() {
        return None;
    }
    match entry {
        SurfaceEntry::Both { import, .. } if !import.is_empty() => Some(import.clone()),
        SurfaceEntry::Both { path, .. } | SurfaceEntry::Specifier { path, .. } => {
            let looks_like_path = path.starts_with('.') || path.starts_with('/') || path.contains('/');
            if !looks_like_path && !path.is_empty() {
                Some(path.clone())
            } else {
                root_pkg.map(str::to_string)
            }
        }
    }
}

fn root_package_name(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("package.json")).ok()?;
    let pkg: Value = serde_json::from_str(&text).ok()?;
    pkg.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

/// First language token of an info string if runnable and not marked `ignore`/`no_run`.
fn runnable_lang<'a>(info: &'a str, languages: &[String]) -> Option<&'a str> {
    let mut words = info.split_whitespace();
    let first = words.next()?;
    let lang = first.split('[').next().unwrap_or(first);
    if !languages.iter().any(|l| l.eq_ignore_ascii_case(lang)) {
        return None;
    }
    if words.any(|w| SKIP_TAGS.iter().any(|skip| w.eq_ignore_ascii_case(skip))) {
        return None;
    }
    Some(lang)
}

/// Whether the snippet contains a standalone `...` placeholder line (outside comments).
fn has_ellipsis_line(code: &str) -> bool {
    code.lines().any(|line| line.trim() == "...")
}

/// The first configured surface whose import specifier appears in an import statement of `code`.
fn matched_surface<'a>(code: &str, imports: &'a [(String, String)]) -> Option<(&'a str, &'a str)> {
    imports
        .iter()
        .find(|(_, spec)| imports_specifier(code, spec))
        .map(|(name, spec)| (name.as_str(), spec.as_str()))
}

/// Whether `code` imports `spec` (quoted specifier, or `import`/`from`/`use` statement).
fn imports_specifier(code: &str, spec: &str) -> bool {
    for quote in ['"', '\''] {
        let prefix = format!("{quote}{spec}");
        for (at, _) in code.match_indices(&prefix) {
            let after = &code[at + prefix.len()..];
            if after.starts_with(quote) || after.starts_with('/') {
                return true;
            }
        }
    }
    code.lines().any(|line| {
        let trimmed = line.trim();
        for keyword in ["from ", "import ", "use "] {
            if let Some(rest) = trimmed.strip_prefix(keyword)
                && let Some(after) = rest.strip_prefix(spec)
                && after.chars().next().is_none_or(|c| matches!(c, ' ' | '\t' | '.' | ':' | ';' | ',' | '{'))
            {
                return true;
            }
        }
        false
    })
}

fn diagnostics_for(diagnostics: &[Value], id: &str) -> Result<Vec<String>> {
    diagnostics
        .iter()
        .filter(|d| d.get("id").and_then(Value::as_str) == Some(id))
        .map(|d| {
            let msg = d
                .get("message")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::internal("compile answered a diagnostic without a message"))?;
            let line = match d.get("line") {
                None | Some(Value::Null) => "1".to_string(),
                Some(Value::Number(n)) => n.to_string(),
                Some(_) => return Err(Error::internal("compile answered a line that is not a number")),
            };
            Ok(format!("{msg} (block line {line})"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_surface_imports_across_quoted_and_unquoted_languages() {
        assert!(imports_specifier("import { x } from '@acme/sdk';", "@acme/sdk"));
        assert!(imports_specifier("import { x } from '@acme/sdk/sub';", "@acme/sdk"));
        assert!(!imports_specifier("import { x } from '@acme/sdk-extra';", "@acme/sdk"));
        assert!(imports_specifier("from acme import Client", "acme"));
        assert!(imports_specifier("import acme.client", "acme"));
        assert!(imports_specifier("use my_crate::Client;", "my_crate"));
        assert!(!imports_specifier("use my_crate_other::Client;", "my_crate"));
    }

    #[test]
    fn skips_non_runnable_ignored_and_elliptical_blocks() {
        let langs = vec!["ts".to_string(), "typescript".to_string()];
        assert_eq!(runnable_lang("ts", &langs), Some("ts"));
        assert_eq!(runnable_lang("typescript twoslash", &langs), Some("typescript"));
        assert_eq!(runnable_lang("ts ignore", &langs), None);
        assert_eq!(runnable_lang("ts no_run", &langs), None);
        assert_eq!(runnable_lang("json", &langs), None);
        assert!(has_ellipsis_line("const a = 1;\n...\nconst b = 2;"));
        assert!(!has_ellipsis_line("const a = [...xs];\n// ..."));
    }
}
