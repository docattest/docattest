//! `surfaces`: the entries examples import and facts read, by name.

use serde_json::{Map, Value};

use super::Fields;
use crate::text::trim;

/// What an entry may be. What a string names, or what is imported from which file, is the
/// adapter's to say, and it says so when it opens the project.
const SHAPE: &str = "a string, or { \"path\": \"<file>\", \"import\"?: \"<name>\", \"internal\"?: true }";

/// A surface as written in the config: a string, or an object naming a path with an optional
/// import name and an optional `internal` flag. Internal surfaces are available to facts,
/// `audit`, `impact` and `where`, but excluded from public API coverage and `uncovered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceEntry {
    Specifier { path: String, internal: bool },
    Both { import: String, path: String, internal: bool },
}

impl SurfaceEntry {
    /// Whether this surface is internal (fact-only, excluded from public API coverage).
    pub fn is_internal(&self) -> bool {
        match self {
            SurfaceEntry::Specifier { internal, .. } | SurfaceEntry::Both { internal, .. } => *internal,
        }
    }

    /// The entry as the adapter protocol expects it (`internal` is core-only).
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            SurfaceEntry::Specifier { path, .. } => serde_json::json!(path),
            SurfaceEntry::Both { import, path, .. } => serde_json::json!({ "import": import, "path": path }),
        }
    }
}

pub(super) fn read_surfaces(fields: &mut Fields) -> Vec<(String, SurfaceEntry)> {
    let Some(raw) = fields.object.get("surfaces") else {
        return Vec::new();
    };
    let Value::Object(value) = raw else {
        fields.problems.push("\"surfaces\" must be an object".into());
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, entry) in value.iter() {
        match surface_entry(entry) {
            Some(surface) => out.push((name.to_string(), surface)),
            None => fields.problems.push(format!("\"surfaces.{name}\" must be {SHAPE}")),
        }
    }
    out
}

fn surface_entry(entry: &Value) -> Option<SurfaceEntry> {
    match entry {
        Value::String(s) => {
            (!trim(s).is_empty()).then(|| SurfaceEntry::Specifier { path: s.clone(), internal: false })
        }
        Value::Object(o) => object_entry(o),
        _ => None,
    }
}

fn object_entry(o: &Map<String, Value>) -> Option<SurfaceEntry> {
    if !o.keys().all(|k| matches!(k.as_str(), "import" | "path" | "internal")) {
        return None;
    }
    let path = non_empty_string(o.get("path")?)?;
    let import = optional_non_empty_string(o.get("import"))?;
    let internal = optional_bool(o.get("internal"))?;
    if import.is_none() && internal.is_none() {
        return None;
    }
    let internal = internal.unwrap_or(false);
    match import {
        Some(import) => Some(SurfaceEntry::Both { import, path, internal }),
        None => Some(SurfaceEntry::Specifier { path, internal }),
    }
}

fn optional_non_empty_string(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        None => Some(None),
        Some(v) => non_empty_string(v).map(Some),
    }
}

fn optional_bool(value: Option<&Value>) -> Option<Option<bool>> {
    match value {
        None => Some(None),
        Some(Value::Bool(b)) => Some(Some(*b)),
        Some(_) => None,
    }
}

fn non_empty_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}
