//! A manifest's text as data, by the format its name says: JSON (`.json`), TOML (`.toml`)
//! or `go.mod`. Every manifest is an object at the top, whatever its format, so that a fact
//! reads a field of any of them the same way.

use serde_json::{Map, Number, Value};

use super::go_mod::parse_go_mod;
use crate::json::parse_json;

/// The data `text`, the contents of `file`, holds; or why it holds none, as a phrase that
/// follows the manifest's name.
pub(super) fn parse_manifest(file: &str, text: &str) -> Result<Value, String> {
    let name = file.rsplit('/').next().unwrap_or(file);
    if name == "go.mod" {
        return parse_go_mod(text);
    }
    match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("json") => json_object(text),
        Some("toml") => toml_table(text),
        _ => Err("not a format docattest reads: JSON (.json), TOML (.toml) or go.mod".to_string()),
    }
}

fn json_object(text: &str) -> Result<Value, String> {
    match parse_json(text).map_err(|why| format!("not valid JSON: {why}"))? {
        object @ Value::Object(_) => Ok(object),
        _ => Err("not a JSON object".to_string()),
    }
}

fn toml_table(text: &str) -> Result<Value, String> {
    let table = text.parse::<toml::Table>().map_err(|e| {
        let line = e.span().map_or(1, |span| text[..span.start].matches('\n').count() + 1);
        format!("not valid TOML: {} (line {line})", e.message())
    })?;
    Ok(Value::Object(toml_entries(table)))
}

fn toml_entries(table: toml::Table) -> Map<String, Value> {
    table.into_iter().map(|(key, value)| (key, toml_value(value))).collect()
}

/// A TOML value as JSON. A date or time is the text TOML wrote it as; a float JSON cannot
/// hold (`inf`, `nan`) is its text too.
fn toml_value(value: toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(n) => Value::Number(n.into()),
        toml::Value::Float(x) => {
            Number::from_f64(x).map_or_else(|| Value::String(x.to_string()), Value::Number)
        }
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(when) => Value::String(when.to_string()),
        toml::Value::Array(items) => Value::Array(items.into_iter().map(toml_value).collect()),
        toml::Value::Table(table) => Value::Object(toml_entries(table)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_and_toml_are_read_in_the_order_they_are_written() {
        let package = parse_manifest("a/package.json", r#"{"name":"x","engines":{"node":">=22"}}"#).unwrap();
        assert_eq!(package, json!({ "name": "x", "engines": { "node": ">=22" } }));
        let toml = "[project]\nname = \"x\"\nrequires-python = \">=3.10\"\n\n[project.scripts]\nb = \"m:b\"\na = \"m:a\"\n";
        let pyproject = parse_manifest("pyproject.toml", toml).unwrap();
        let scripts: Vec<&String> = pyproject["project"]["scripts"].as_object().unwrap().keys().collect();
        assert_eq!(scripts, ["b", "a"]);
        assert_eq!(pyproject["project"]["requires-python"], ">=3.10");
        let values =
            parse_manifest("v.toml", "n = 3\nx = 1.5\nwhen = 2024-01-02\nall = [true, \"s\"]\n").unwrap();
        assert_eq!(values, json!({ "n": 3, "x": 1.5, "when": "2024-01-02", "all": [true, "s"] }));
    }

    #[test]
    fn a_file_that_is_not_an_object_of_a_known_format_is_refused() {
        assert_eq!(parse_manifest("a.json", "[1]"), Err("not a JSON object".to_string()));
        assert_eq!(
            parse_manifest("a.json", "{\"a\":"),
            Err("not valid JSON: EOF while parsing a value at line 1 column 5".to_string())
        );
        assert_eq!(
            parse_manifest("a.toml", "a = 1\nb = \n"),
            Err("not valid TOML: string values must be quoted, expected literal string (line 2)".to_string())
        );
        assert_eq!(
            parse_manifest("a.yaml", "a: 1"),
            Err("not a format docattest reads: JSON (.json), TOML (.toml) or go.mod".to_string())
        );
    }
}
