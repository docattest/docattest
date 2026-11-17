//! The protocol's JSON Schemas (`protocol/schema/` in the repository, carried in the binary)
//! and a validator for the part of draft 2020-12 they use: `type`, `properties`, `required`,
//! `additionalProperties`, `items`, `enum`, `const`, `anyOf`, `minimum`, `minLength`, and
//! `$ref` within and between files. A keyword it does not know is ignored, so a schema that
//! starts using one must be met with a rule here.

mod exchange;

use serde_json::{Map, Value};

/// The schema files, by the name a `$ref` gives them.
const FILES: [(&str, &str); 8] = [
    ("agent.json", include_str!("../../protocol/schema/agent.json")),
    ("attestations.json", include_str!("../../protocol/schema/attestations.json")),
    ("kit.json", include_str!("../../protocol/schema/kit.json")),
    ("language.json", include_str!("../../protocol/schema/language.json")),
    ("messages.json", include_str!("../../protocol/schema/messages.json")),
    ("methods.json", include_str!("../../protocol/schema/methods.json")),
    ("project.json", include_str!("../../protocol/schema/project.json")),
    ("results.json", include_str!("../../protocol/schema/results.json")),
];

/// The file that holds every method's params and result.
pub const METHODS: &str = "methods.json";

/// The protocol's schemas, parsed.
pub struct Schemas {
    files: Vec<(&'static str, Value)>,
}

impl Schemas {
    /// The schemas this binary was built with.
    pub fn embedded() -> Schemas {
        let parse = |(name, text): (&'static str, &str)| {
            let value = serde_json::from_str(text).unwrap_or_else(|e| panic!("{name} is not JSON: {e}"));
            (name, value)
        };
        Schemas { files: FILES.into_iter().map(parse).collect() }
    }

    /// What is wrong with `value` under `#/$defs/<definition>` of `file`; nothing when it fits.
    pub fn check(&self, value: &Value, definition: &str, file: &str) -> Vec<String> {
        match self.definition(definition, file) {
            Some(schema) => self.problems(value, schema, file, "$"),
            None => vec![format!("no schema \"{definition}\" in {file}")],
        }
    }

    /// The definition `name` of `file`, when there is one.
    fn definition(&self, name: &str, file: &str) -> Option<&Value> {
        self.file(file)?.get("$defs")?.get(name)
    }

    fn file(&self, name: &str) -> Option<&Value> {
        self.files.iter().find(|(file, _)| *file == name).map(|(_, value)| value)
    }

    /// The schema a `$ref` names, and its file: `#/$defs/x` in `file`, or `other.json#/$defs/x`.
    fn resolve<'a>(&'a self, reference: &'a str, file: &'a str) -> (Option<&'a Value>, &'a str) {
        let (name, pointer) = reference.split_once('#').unwrap_or((reference, ""));
        let target = if name.is_empty() { file } else { name };
        (self.file(target).and_then(|doc| doc.pointer(pointer)), target)
    }

    /// What is wrong with `value` at `at` under `schema` (of `file`); a wrong type ends it.
    fn problems(&self, value: &Value, schema: &Value, file: &str, at: &str) -> Vec<String> {
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            return match self.resolve(reference, file) {
                (Some(target), target_file) => self.problems(value, target, target_file, at),
                (None, _) => vec![format!("{at}: the schema names {reference}, which there is not")],
            };
        }
        let mut out = value_problems(value, schema, at);
        if let Some(wrong_type) = type_problem(value, schema, at) {
            out.push(wrong_type);
            return out;
        }
        out.extend(self.alternative_problems(value, schema, file, at));
        out.extend(bound_problems(value, schema, at));
        if let Value::Object(object) = value {
            out.extend(self.object_problems(object, schema, file, at));
        }
        if let (Value::Array(items), Some(each)) = (value, schema.get("items")) {
            let item = |(k, item)| self.problems(item, each, file, &format!("{at}[{k}]"));
            out.extend(items.iter().enumerate().flat_map(item));
        }
        out
    }

    /// `anyOf`: the value fits one of the alternatives; when none, the nearest one's problems.
    fn alternative_problems(&self, value: &Value, schema: &Value, file: &str, at: &str) -> Vec<String> {
        let Some(options) = schema.get("anyOf").and_then(Value::as_array) else { return vec![] };
        let tried: Vec<Vec<String>> =
            options.iter().map(|option| self.problems(value, option, file, at)).collect();
        match tried.into_iter().min_by_key(Vec::len) {
            Some(nearest) if !nearest.is_empty() => {
                vec![format!("{at}: matches none of the alternatives ({})", nearest.join("; "))]
            }
            _ => vec![],
        }
    }

    /// `required`, `properties` and `additionalProperties`.
    fn object_problems(
        &self,
        object: &Map<String, Value>,
        schema: &Value,
        file: &str,
        at: &str,
    ) -> Vec<String> {
        let required = schema.get("required").and_then(Value::as_array).into_iter().flatten();
        let mut out: Vec<String> = required
            .filter_map(Value::as_str)
            .filter(|key| !object.contains_key(*key))
            .map(|key| format!("{at}: lacks \"{key}\""))
            .collect();
        let properties = schema.get("properties");
        let extra = schema.get("additionalProperties");
        for (key, item) in object {
            let place = format!("{at}.{key}");
            match (properties.and_then(|p| p.get(key)), extra) {
                (Some(own), _) => out.extend(self.problems(item, own, file, &place)),
                (None, Some(Value::Bool(false))) => {
                    out.push(format!("{at}: has \"{key}\", which the schema does not know"));
                }
                (None, Some(extra @ Value::Object(_))) => {
                    out.extend(self.problems(item, extra, file, &place))
                }
                (None, _) => {}
            }
        }
        out
    }
}

/// `const` and `enum`: the value is one the schema allows.
fn value_problems(value: &Value, schema: &Value, at: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(constant) = schema.get("const").filter(|constant| *constant != value) {
        out.push(format!("{at}: is not {constant}"));
    }
    if let Some(allowed) =
        schema.get("enum").and_then(Value::as_array).filter(|allowed| !allowed.contains(value))
    {
        let names: Vec<String> = allowed.iter().map(plain).collect();
        out.push(format!("{at}: {value} is not one of {}", names.join(", ")));
    }
    out
}

/// `type`: one name, or a list of names.
fn type_problem(value: &Value, schema: &Value, at: &str) -> Option<String> {
    let allowed: Vec<&str> = match schema.get("type")? {
        Value::String(one) => vec![one],
        Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
        _ => return None,
    };
    if allowed.iter().any(|name| is_of_type(value, name)) {
        return None;
    }
    Some(format!("{at}: {value} is not of type {}", allowed.join("/")))
}

/// Whether `value` is of the JSON type `name`; an integer is a number with no fraction.
fn is_of_type(value: &Value, name: &str) -> bool {
    match name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "number" => value.is_number(),
        "integer" => value.as_f64().is_some_and(|n| n.fract() == 0.0),
        _ => false,
    }
}

/// `minimum` for a number, `minLength` for a string.
fn bound_problems(value: &Value, schema: &Value, at: &str) -> Vec<String> {
    let minimum = schema.get("minimum").and_then(Value::as_f64);
    if let (Some(number), Some(minimum)) = (value.as_f64(), minimum)
        && number < minimum
    {
        return vec![format!("{at}: {value} is below {}", schema["minimum"])];
    }
    let min_length = schema.get("minLength").and_then(Value::as_u64);
    if let (Some(text), Some(min_length)) = (value.as_str(), min_length)
        && (text.chars().count() as u64) < min_length
    {
        return vec![format!("{at}: is shorter than {min_length}")];
    }
    vec![]
}

/// A value as a list names it: a string as it is, anything else as JSON.
fn plain(value: &Value) -> String {
    value.as_str().map_or_else(|| value.to_string(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn it_refuses_what_the_schemas_refuse() {
        let schemas = Schemas::embedded();
        let check = |value: Value, definition: &str, file: &str| schemas.check(&value, definition, file);
        let observation = json!({ "text": "a", "description": "string a", "kind": "text" });
        assert_eq!(check(observation.clone(), "Observation", "results.json"), Vec::<String>::new());
        let with = |key: &str, value: Value| {
            let mut changed = observation.clone();
            changed[key] = value;
            changed
        };
        assert_eq!(
            check(json!({ "text": "a" }), "Observation", "results.json"),
            ["$: lacks \"description\""]
        );
        assert!(!check(with("kind", json!("string")), "Observation", "results.json").is_empty());
        assert!(!check(with("items", json!([1])), "Observation", "results.json").is_empty());
        assert_eq!(
            check(with("more", json!(1)), "Observation", "results.json"),
            ["$: has \"more\", which the schema does not know"]
        );
        let suite = json!({ "title": "T", "describes": [null], "line": 1 });
        assert!(
            !check(json!({ "ok": true, "tests": [], "suites": [suite] }), "tests.result", METHODS).is_empty()
        );
        let named = json!({ "title": "T", "describes": [null], "line": 1, "kind": "a class" });
        assert!(
            check(json!({ "ok": true, "tests": [], "suites": [named] }), "tests.result", METHODS).is_empty()
        );
        assert!(!check(json!({ "id": 0, "method": "x" }), "Request", "messages.json").is_empty());
        assert!(
            !check(json!({ "id": 1, "result": {}, "error": {} }), "Response", "messages.json").is_empty()
        );
        assert_eq!(check(json!(1), "Nothing", METHODS), ["no schema \"Nothing\" in methods.json"]);
    }

    #[test]
    fn every_reference_names_a_schema_there_is() {
        let schemas = Schemas::embedded();
        let mut references = Vec::new();
        for (file, value) in &schemas.files {
            collect_references(value, file, &mut references);
        }
        assert!(references.len() > 40);
        for (file, reference) in references {
            assert!(schemas.resolve(&reference, file).0.is_some(), "{file}: {reference}");
        }
    }

    fn collect_references(value: &Value, file: &'static str, out: &mut Vec<(&'static str, String)>) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(reference)) = object.get("$ref") {
                    out.push((file, reference.clone()));
                }
                object.values().for_each(|v| collect_references(v, file, out));
            }
            Value::Array(items) => items.iter().for_each(|v| collect_references(v, file, out)),
            _ => {}
        }
    }

    #[test]
    fn types_bounds_and_values_are_checked_as_draft_2020_12_says() {
        let schemas = Schemas::embedded();
        let at = |value: Value, schema: Value| schemas.problems(&value, &schema, METHODS, "$");
        assert!(at(json!(2.0), json!({ "type": "integer" })).is_empty());
        assert_eq!(at(json!(2.5), json!({ "type": "integer" })), ["$: 2.5 is not of type integer"]);
        assert!(at(json!(null), json!({ "type": ["string", "null"] })).is_empty());
        assert_eq!(at(json!(0), json!({ "minimum": 1 })), ["$: 0 is below 1"]);
        assert_eq!(at(json!("é"), json!({ "minLength": 2 })), ["$: is shorter than 2"]);
        assert_eq!(at(json!("c"), json!({ "enum": ["a", "b"] })), ["$: \"c\" is not one of a, b"]);
        assert_eq!(at(json!(2), json!({ "const": 1 })), ["$: is not 1"]);
        let either = json!({ "anyOf": [{ "type": "object", "required": ["a"] }, { "type": "string" }] });
        assert!(at(json!("x"), either.clone()).is_empty());
        assert_eq!(at(json!({}), either), ["$: matches none of the alternatives ($: lacks \"a\")"]);
        assert_eq!(
            at(json!({ "a": 1 }), json!({ "additionalProperties": { "type": "string" } })),
            ["$.a: 1 is not of type string"]
        );
    }
}
