//! Built-in `"none"` adapter: runs in-process with zero child processes.
//!
//! Supports core-native attestations without a language runtime: `console check`, `md check`,
//! and facts that read `matched()`, `manifestField()`, `file()`, `manifest()` or `snapshot()`.

use serde_json::{Map, Value, json};

use crate::api::errors::{Error, Result};
use crate::api::field::as_json_writes;
use crate::fact_files::calls::{Call, named_in};
use crate::text::trim;

/// In-process state for an open `"none"` session.
#[derive(Default)]
pub(crate) struct BuiltinNone {
    snapshots: Map<String, Value>,
    manifests: Map<String, Value>,
    texts: Map<String, Value>,
    matches: Map<String, Value>,
    manifest_fields: Map<String, Value>,
}

impl BuiltinNone {
    /// Answer one protocol request synchronously.
    pub(crate) fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        match method {
            "hello" => Ok(hello_reply()),
            "open" => {
                self.open(&params);
                Ok(Value::Null)
            }
            "close" => Ok(Value::Null),
            "display" => Ok(display_values(&params)),
            "evaluate" => Ok(self.evaluate_facts(&params)),
            other => neutral_query(other, &params),
        }
    }

    fn open(&mut self, params: &Value) {
        self.snapshots = object_field(params.get("snapshots").and_then(|s| s.get("data")));
        self.manifests = object_field(params.get("manifests"));
        self.texts = object_field(params.get("texts"));
        self.matches = object_field(params.get("matches"));
        self.manifest_fields = object_field(params.get("manifestFields"));
    }

    fn evaluate_facts(&self, params: &Value) -> Value {
        let mut out = Map::new();
        let Some(facts) = params.get("facts").and_then(Value::as_array) else {
            return Value::Object(out);
        };
        for fact in facts {
            let Some(id) = fact.get("id").and_then(Value::as_str) else { continue };
            let expr = fact.get("expression").and_then(Value::as_str).unwrap_or("");
            let eval = match self.eval_expression(expr) {
                Ok((val, is_text)) => match observation_json(&val, is_text) {
                    Ok(obs) => json!({ "ok": true, "value": obs }),
                    Err(msg) => json!({ "ok": false, "message": format!("fact `{expr}`: {msg}") }),
                },
                Err(msg) => json!({ "ok": false, "message": msg }),
            };
            out.insert(id.to_string(), eval);
        }
        Value::Object(out)
    }

    fn eval_expression(&self, raw: &str) -> std::result::Result<(Value, bool), String> {
        let expr = trim(raw);
        if let Some((base, rest, is_text)) = self.parse_root_call(expr)? {
            let value = walk_accessors(base, rest, expr)?;
            return Ok((value.clone(), is_text && rest.is_empty()));
        }
        Err(format!(
            "fact `{expr}` cannot be evaluated with adapter \"none\": use matched(), manifestField(), file(), manifest() or snapshot(), or configure a language adapter"
        ))
    }

    fn parse_root_call<'a>(
        &'a self,
        expr: &'a str,
    ) -> std::result::Result<Option<(&'a Value, &'a str, bool)>, String> {
        let Some((name, after_name)) = split_ident(expr) else { return Ok(None) };
        let Some(after_open) = after_name.trim_start().strip_prefix('(') else { return Ok(None) };
        let Some(close) = find_close_paren(after_open) else { return Ok(None) };
        let call_text = &expr[..expr.len() - after_open.len() + close + 1];
        let rest = trim(&after_open[close + 1..]);
        if name == "snapshot" {
            let arg = parse_string_arg(trim(&after_open[..close]))
                .ok_or_else(|| format!("fact `{expr}`: snapshot() requires a string literal name"))?;
            let val = self
                .snapshots
                .get(&arg)
                .ok_or_else(|| format!("fact `{expr}`: snapshot \"{arg}\" not found"))?;
            return Ok(Some((val, rest, false)));
        }
        let Some(call) = named_in(call_text).next() else { return Ok(None) };
        let (val, is_text) = self.lookup_call(&call, expr)?;
        Ok(Some((val, rest, is_text)))
    }

    fn lookup_call(&self, call: &Call, expr: &str) -> std::result::Result<(&Value, bool), String> {
        match call {
            Call::Matched { path, pattern } => self
                .matches
                .get(path)
                .and_then(|m| m.get(pattern))
                .map(|v| (v, false))
                .ok_or_else(|| format!("fact `{expr}`: matched(\"{path}\", \"{pattern}\") was not read")),
            Call::ManifestField { path, keys } => {
                self.manifest_fields.get(path).and_then(|m| m.get(keys)).map(|v| (v, false)).ok_or_else(
                    || format!("fact `{expr}`: manifestField(\"{path}\", \"{keys}\") was not read"),
                )
            }
            Call::File { path } => self
                .texts
                .get(path)
                .map(|v| (v, true))
                .ok_or_else(|| format!("fact `{expr}`: file(\"{path}\") was not read")),
            Call::Manifest { path } => self
                .manifests
                .get(path)
                .map(|v| (v, false))
                .ok_or_else(|| format!("fact `{expr}`: manifest(\"{path}\") was not read")),
        }
    }
}

fn neutral_query(method: &str, params: &Value) -> Result<Value> {
    match method {
        "describeSurfaces" => Ok(json!({})),
        "exports" | "docComments" | "inCode" => Ok(json!([])),
        "symbols" => Ok(empty_symbols(params)),
        "assertions" => Ok(vec_of(params, "examples", Value::Bool(false))),
        "docProse" => Ok(vec_of(params, "files", json!([]))),
        "mentions" => Ok(vec_of(params, "spans", Value::Null)),
        "expressions" => Ok(expression_shapes(params)),
        _ => Err(Error::internal(format!("adapter \"none\" does not support method \"{method}\""))),
    }
}

fn hello_reply() -> Value {
    json!({
        "blockLanguages": [],
        "capabilities": ["display"],
        "sourceFiles": { "include": [], "exclude": [] },
        "ignoredNames": [".git", "node_modules", "target", "dist", "build", ".venv", "__pycache__"],
    })
}

fn object_field(value: Option<&Value>) -> Map<String, Value> {
    value.and_then(Value::as_object).cloned().unwrap_or_default()
}

fn vec_of(params: &Value, key: &str, fill: Value) -> Value {
    let n = params.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    Value::Array(vec![fill; n])
}

fn empty_symbols(params: &Value) -> Value {
    let mut out = Map::new();
    if let Some(items) = params.get("attestations").and_then(Value::as_array) {
        for item in items {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                out.insert(id.to_string(), json!([]));
            }
        }
    }
    Value::Object(out)
}

fn expression_shapes(params: &Value) -> Value {
    let items = params.get("expressions").and_then(Value::as_array).cloned().unwrap_or_default();
    let shapes: Vec<Value> = items
        .iter()
        .map(|e| {
            let expr = e.as_str().unwrap_or("");
            json!({ "shape": trim(expr), "names": [] })
        })
        .collect();
    Value::Array(shapes)
}

fn display_values(params: &Value) -> Value {
    let items = params.get("values").and_then(Value::as_array).cloned().unwrap_or_default();
    Value::Array(items.iter().map(display_one).collect())
}

fn display_one(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(|i| Value::String(scalar_text(i))).collect()),
        other => Value::String(scalar_text(other)),
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => as_json_writes(other),
    }
}

fn observation_json(value: &Value, is_text: bool) -> std::result::Result<Value, String> {
    match value {
        Value::Array(items) => {
            if items.iter().any(|i| matches!(i, Value::Object(_) | Value::Array(_))) {
                return Err("list contains objects or nested lists".into());
            }
            let rendered: Vec<String> = items.iter().map(scalar_text).collect();
            let text = rendered.join(", ");
            let description = format!("{} items", rendered.len());
            Ok(json!({ "text": text, "items": rendered, "description": description }))
        }
        Value::Object(_) => Err("result is an object; select a scalar or list field".into()),
        scalar => {
            let text = scalar_text(scalar);
            let mut obj = Map::new();
            obj.insert("text".into(), Value::String(text.clone()));
            obj.insert("description".into(), Value::String(text));
            if is_text {
                obj.insert("kind".into(), Value::String("text".into()));
            }
            Ok(Value::Object(obj))
        }
    }
}

fn split_ident(s: &str) -> Option<(&str, &str)> {
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(s.len());
    (end > 0).then(|| (&s[..end], &s[end..]))
}

fn find_close_paren(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            if c == '\\' && q != '`' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
        } else if matches!(c, '"' | '\'' | '`') {
            quote = Some(c);
        } else if c == ')' {
            return Some(i);
        }
    }
    None
}

fn parse_string_arg(s: &str) -> Option<String> {
    let quote = s.chars().next().filter(|c| matches!(c, '"' | '\'' | '`'))?;
    if s.len() < 2 || !s.ends_with(quote) {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' && quote != '`' {
            out.push(chars.next()?);
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn walk_accessors<'a>(
    mut current: &'a Value,
    mut rest: &str,
    expr: &str,
) -> std::result::Result<&'a Value, String> {
    while !rest.is_empty() {
        let (key, next_rest) = next_accessor(rest).ok_or_else(|| {
            format!(
                "fact `{expr}` uses syntax adapter \"none\" cannot evaluate (\"{rest}\"); use manifestField() or configure a language adapter"
            )
        })?;
        current = step_value(current, &key, expr)?;
        rest = trim(next_rest);
    }
    Ok(current)
}

fn next_accessor(rest: &str) -> Option<(String, &str)> {
    if let Some(after_dot) = rest.strip_prefix('.') {
        let end = after_dot
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(after_dot.len());
        return (end > 0).then(|| (after_dot[..end].to_string(), &after_dot[end..]));
    }
    let after_open = rest.strip_prefix('[')?;
    let close = after_open.find(']')?;
    let inside = trim(&after_open[..close]);
    let key = parse_string_arg(inside)
        .or_else(|| inside.chars().all(|c| c.is_ascii_digit()).then(|| inside.to_string()))?;
    Some((key, &after_open[close + 1..]))
}

fn step_value<'a>(value: &'a Value, key: &str, expr: &str) -> std::result::Result<&'a Value, String> {
    match value {
        Value::Object(map) => map.get(key).ok_or_else(|| format!("fact `{expr}`: key \"{key}\" not found")),
        Value::Array(items) => key
            .parse::<usize>()
            .ok()
            .and_then(|i| items.get(i))
            .ok_or_else(|| format!("fact `{expr}`: index \"{key}\" out of bounds")),
        _ => Err(format!("fact `{expr}`: cannot read \"{key}\" from scalar value")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_manifest_matched_file_and_snapshot() {
        let mut adapter = BuiltinNone::default();
        adapter.open(&json!({
            "manifests": { "package.json": { "version": "1.2.3", "engines": { "node": ">=22" }, "tags": ["a", "b"] } },
            "texts": { "LICENSE": "MIT License" },
            "matches": { "Dockerfile": { "^USER (\\d+)$": "10001" } },
            "manifestFields": { "package.json": { "engines.node": ">=22" } },
            "snapshots": { "data": { "regions": ["eu", "us"] } }
        }));
        let mut eval = |expr: &str| -> Value {
            let res =
                adapter.call("evaluate", json!({ "facts": [{ "id": "1", "expression": expr }] })).unwrap();
            res["1"].clone()
        };
        assert_eq!(eval(r#"manifest("package.json").version"#)["value"]["text"], "1.2.3");
        assert_eq!(eval(r#"manifest("package.json")["engines"]["node"]"#)["value"]["text"], ">=22");
        assert_eq!(eval(r#"manifestField("package.json", "engines.node")"#)["value"]["text"], ">=22");
        assert_eq!(eval(r#"matched("Dockerfile", "^USER (\\d+)$")"#)["value"]["text"], "10001");
        assert_eq!(eval(r#"file("LICENSE")"#)["value"]["kind"], "text");
        assert_eq!(eval(r#"snapshot("regions")"#)["value"]["items"], json!(["eu", "us"]));
        assert_eq!(eval(r#"manifest("package.json").tags[1]"#)["value"]["text"], "b");
        assert_eq!(eval("RETRY_DEFAULTS.maxAttempts")["ok"], false);
    }
}
