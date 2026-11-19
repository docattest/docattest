//! `field=`: a fact about one value of the JSON, TOML or YAML a fenced block holds, at a path
//! of keys (`ports.0.port`). The block is parsed as a manifest of its language is.

use serde_json::Value;

use super::quoted::{first_outside_quotes, unquoted};
use crate::api::attestation::FactTarget;
use crate::api::field::{FieldValue, as_json_writes};
use crate::counted::counted;
use crate::manifests::format::parse_data;
use crate::markdown::model::{Block, BlockKind};
use crate::text::{is_js_space, words};

/// A path into a block's data: the keys as written, and each key's text. A key of digits
/// also picks a list's item by its index, counted from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPath {
    pub written: String,
    keys: Vec<String>,
}

/// The path `written` names, as `field=` gives it; or why it names none, as a phrase.
pub fn parse_field_path(written: &str) -> Result<FieldPath, String> {
    FieldPath::parse(written).map_err(|why| format!("field={why}"))
}

impl FieldPath {
    /// The path `written` names: keys parted by dots, a key that holds a dot or a space in
    /// quotes (`servers."eu.west".port`); or why it names none, as a phrase that starts with
    /// the path. A fact's `manifestField("<file>", "<path>")` names a path in a manifest so
    /// too.
    pub fn parse(written: &str) -> Result<FieldPath, String> {
        let mut keys = Vec::new();
        let mut rest = written;
        loop {
            let end = first_outside_quotes(rest, |c| c == '.').unwrap_or(rest.len());
            let key = &rest[..end];
            if key.is_empty() {
                return Err(format!("{written} has an empty key"));
            }
            keys.push(unquoted(key).unwrap_or(key).to_string());
            if end == rest.len() {
                return Ok(FieldPath { written: written.to_string(), keys });
            }
            rest = &rest[end + 1..];
        }
    }

    /// The value at this path in `data`; or, where the path stops, what is there instead.
    pub fn value_in<'v>(&self, data: &'v Value) -> Result<&'v Value, String> {
        value_at(data, &self.keys)
    }
}

/// Bind `path` in the fenced block `next`: its value as a scalar, or a list of scalars as a
/// set. Anything else is an error that says what is there instead.
pub fn bind_field(
    at: &str,
    path: &FieldPath,
    ordered: bool,
    next: Option<&Block>,
) -> Result<FactTarget, String> {
    let written = &path.written;
    let Some((start, info, text)) = next.and_then(fenced) else {
        return Err(format!("{at} has field={written} but no fenced block follows it"));
    };
    let language = words(info).first().copied().unwrap_or_default();
    let Some(parsed) = parse_data(language, text) else {
        let shown = if language.is_empty() { "has no language" } else { &format!("is {language}") };
        return Err(format!(
            "{at} has field={written} but the block after it {shown}, not json, toml or yaml"
        ));
    };
    let block = format!("the {language} block at L{start}");
    let data = parsed.map_err(|why| format!("{at}: {block} cannot be read: {why}"))?;
    let value = path.value_in(&data).map_err(|why| format!("{at}: {block} has no {written}: {why}"))?;
    target_of(value, written, ordered).map_err(|why| format!("{at}: {written} in {block} {why}"))
}

/// A fenced block's first line, info string and text.
fn fenced(block: &Block) -> Option<(usize, &str, &str)> {
    match &block.kind {
        BlockKind::Code { info, value } => Some((block.start, info, value)),
        _ => None,
    }
}

/// The value at `keys` in `data`; or, where the path stops, what is there instead.
fn value_at<'v>(data: &'v Value, keys: &[String]) -> Result<&'v Value, String> {
    let mut value = data;
    for (depth, key) in keys.iter().enumerate() {
        let inside = match value {
            Value::Object(entries) => entries.get(key),
            Value::Array(items) => key.parse::<usize>().ok().and_then(|index| items.get(index)),
            _ => None,
        };
        let Some(inside) = inside else {
            let place = if depth == 0 { "the top level".to_string() } else { shown_path(&keys[..depth]) };
            return Err(format!("{place} {}", what_is(value)));
        };
        value = inside;
    }
    Ok(value)
}

/// What a value holds, for a path that cannot go on through it.
fn what_is(value: &Value) -> String {
    match value {
        Value::Object(entries) if entries.is_empty() => "has no keys".to_string(),
        Value::Object(entries) => {
            let keys: Vec<&str> = entries.keys().map(String::as_str).collect();
            format!("has keys {}", keys.join(", "))
        }
        Value::Array(items) => format!("is a list of {}", counted(items.len(), "item")),
        scalar => format!("is {}, not an object or list", as_json_writes(scalar)),
    }
}

/// Keys as a path is written, a key that a dot or a space would split in quotes.
fn shown_path(keys: &[String]) -> String {
    let shown = keys.iter().map(|key| {
        let plain = !key.is_empty() && !key.contains(|c: char| c == '.' || is_js_space(c));
        if plain { key.clone() } else { format!("\"{key}\"") }
    });
    shown.collect::<Vec<_>>().join(".")
}

/// The target a field's value is: a scalar, or a list of scalars.
fn target_of(value: &Value, written: &str, ordered: bool) -> Result<FactTarget, String> {
    let field = |value: FieldValue| FactTarget::Field { path: written.to_string(), value };
    if is_scalar(value) {
        if ordered {
            return Err("is a single value; ordered applies to a list".to_string());
        }
        return Ok(field(FieldValue::Scalar(value.clone())));
    }
    let Value::Array(items) = value else {
        return Err(format!("{}; name a field inside it", what_is(value)));
    };
    if !items.iter().all(is_scalar) {
        return Err("is a list of objects or lists; name a field of one of its items".to_string());
    }
    Ok(field(FieldValue::List(items.clone())))
}

/// A string, a number, `true`, `false` or `null`.
fn is_scalar(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse::parse_page;
    use serde_json::json;

    fn keys(written: &str) -> Result<Vec<String>, String> {
        parse_field_path(written).map(|path| path.keys)
    }

    #[test]
    fn a_path_is_keys_parted_by_dots_outside_quotes() {
        assert_eq!(keys("ports.0.port"), Ok(vec!["ports".into(), "0".into(), "port".into()]));
        assert_eq!(
            keys("servers.\"eu.west\".port"),
            Ok(vec!["servers".into(), "eu.west".into(), "port".into()])
        );
        assert_eq!(keys("'build options'"), Ok(vec!["build options".into()]));
        assert_eq!(keys("a..b"), Err("field=a..b has an empty key".into()));
        assert_eq!(keys("a."), Err("field=a. has an empty key".into()));
    }

    fn bound(fence: &str, written: &str, ordered: bool) -> Result<FactTarget, String> {
        let page = parse_page(&format!("Job:\n\n{fence}")).unwrap_or_default();
        let path = parse_field_path(written)?;
        bind_field("fact at L1", &path, ordered, page.flow.last())
    }

    const JOB: &str = "```json\n{\"image\": \"app:1\", \"ports\": [{\"port\": 2222}, {\"port\": 80}], \"tags\": [\"a\", 1, true]}\n```";

    fn scalar(path: &str, value: Value) -> Result<FactTarget, String> {
        Ok(FactTarget::Field { path: path.into(), value: FieldValue::Scalar(value) })
    }

    #[test]
    fn a_field_binds_its_scalar_or_its_list_of_scalars_as_the_block_holds_them() {
        assert_eq!(bound(JOB, "ports.0.port", false), scalar("ports.0.port", json!(2222)));
        assert_eq!(bound(JOB, "image", false), scalar("image", json!("app:1")));
        let tags = FieldValue::List(vec![json!("a"), json!(1), json!(true)]);
        assert_eq!(bound(JOB, "tags", true), Ok(FactTarget::Field { path: "tags".into(), value: tags }));
        let toml = "```toml\n[server]\nport = 8080\nratio = 1.0\n\"eu.west\" = { zone = \"b\" }\n```";
        assert_eq!(bound(toml, "server.port", false), scalar("server.port", json!(8080)));
        assert_eq!(bound(toml, "server.ratio", false), scalar("server.ratio", json!(1.0)));
        assert_eq!(
            bound(toml, "server.\"eu.west\".zone", false),
            scalar("server.\"eu.west\".zone", json!("b"))
        );
        let yaml = "```yml\nservices:\n  web:\n    image: nginx\n    debug: null\n```";
        assert_eq!(bound(yaml, "services.web.image", false), scalar("services.web.image", json!("nginx")));
        assert_eq!(bound(yaml, "services.web.debug", false), scalar("services.web.debug", Value::Null));
    }

    #[test]
    fn a_path_that_is_not_there_says_what_is() {
        let err = |written: &str| bound(JOB, written, false).unwrap_err();
        assert_eq!(
            err("port"),
            "fact at L1: the json block at L3 has no port: the top level has keys image, ports, tags"
        );
        assert_eq!(
            err("ports.2.port"),
            "fact at L1: the json block at L3 has no ports.2.port: ports is a list of 2 items"
        );
        assert_eq!(
            err("ports.0.prot"),
            "fact at L1: the json block at L3 has no ports.0.prot: ports.0 has keys port"
        );
        assert_eq!(
            err("image.tag"),
            "fact at L1: the json block at L3 has no image.tag: image is app:1, not an object or list"
        );
        assert_eq!(
            err("ports"),
            "fact at L1: ports in the json block at L3 is a list of objects or lists; name a field of one of its items"
        );
        assert_eq!(
            err("ports.0"),
            "fact at L1: ports.0 in the json block at L3 has keys port; name a field inside it"
        );
        assert_eq!(
            bound(JOB, "image", true).unwrap_err(),
            "fact at L1: image in the json block at L3 is a single value; ordered applies to a list"
        );
    }

    #[test]
    fn the_block_must_be_data_that_parses() {
        assert_eq!(
            bound("```ts\nconst x = 1;\n```", "x", false).unwrap_err(),
            "fact at L1 has field=x but the block after it is ts, not json, toml or yaml"
        );
        assert_eq!(
            bound("```\nx: 1\n```", "x", false).unwrap_err(),
            "fact at L1 has field=x but the block after it has no language, not json, toml or yaml"
        );
        assert_eq!(
            bound("```json\n{\"a\":\n```", "a", false).unwrap_err(),
            "fact at L1: the json block at L3 cannot be read: not valid JSON: EOF while parsing a value at line 1 column 5"
        );
        assert_eq!(
            bound("Not a block.", "a", false).unwrap_err(),
            "fact at L1 has field=a but no fenced block follows it"
        );
    }
}
