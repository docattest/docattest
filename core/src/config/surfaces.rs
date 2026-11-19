//! `surfaces`: the entries examples import and facts read, by name.

use serde_json::Value;

use super::{DEFAULT_SURFACE, Fields};
use crate::text::trim;

/// What an entry may be. What a string names, or what is imported from which file, is the
/// adapter's to say, and it says so when it opens the project.
const SHAPE: &str = "a string, or { \"import\": \"<name>\", \"path\": \"<file>\" }";

/// A surface as written in the config: a string, or an object naming an import and a path.
/// What either means is the adapter's to say; the core passes it on as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceEntry {
    Specifier(String),
    Both { import: String, path: String },
}

impl SurfaceEntry {
    /// The entry as the config wrote it.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            SurfaceEntry::Specifier(written) => serde_json::json!(written),
            SurfaceEntry::Both { import, path } => serde_json::json!({ "import": import, "path": path }),
        }
    }
}

pub(super) fn read_surfaces(fields: &mut Fields) -> Vec<(String, SurfaceEntry)> {
    let Some(Value::Object(value)) = fields.object.get("surfaces") else {
        fields.problems.push("\"surfaces\" must name a \"default\" surface".into());
        return Vec::new();
    };
    if value.get(DEFAULT_SURFACE).is_none() {
        fields.problems.push("\"surfaces\" must name a \"default\" surface".into());
        return Vec::new();
    }
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
        Value::String(s) => (!trim(s).is_empty()).then(|| SurfaceEntry::Specifier(s.clone())),
        Value::Object(o) => {
            let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
            keys.sort_by(|a, b| crate::text::compare_utf16(a, b));
            if keys != ["import", "path"] {
                return None;
            }
            match (o.get("import"), o.get("path")) {
                (Some(Value::String(import)), Some(Value::String(path)))
                    if !import.is_empty() && !path.is_empty() =>
                {
                    Some(SurfaceEntry::Both { import: import.clone(), path: path.clone() })
                }
                _ => None,
            }
        }
        _ => None,
    }
}
