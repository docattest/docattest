//! A YAML file as data, read by yaml-rust2 (YAML 1.2, core schema), as the frontmatter of a
//! page is. Aliases stand for the node they name; a merge key (`<<`) is a key like any other,
//! since YAML 1.2 has none.

use serde_json::{Map, Number, Value};
use yaml_rust2::{Yaml, YamlLoader};

/// The mapping a YAML file holds; or why it holds none, as a phrase.
pub(super) fn parse_yaml(text: &str) -> Result<Value, String> {
    let documents = YamlLoader::load_from_str(text)
        .map_err(|e| format!("not valid YAML: {} (line {})", e.info(), e.marker().line()))?;
    match &documents[..] {
        [mapping @ Yaml::Hash(_)] => yaml_value(mapping),
        [_, _, ..] => Err("holds more than one YAML document".to_string()),
        _ => Err("not a YAML mapping".to_string()),
    }
}

/// A YAML node as JSON. A float JSON cannot hold (`.inf`, `.nan`) is its text, as in TOML.
fn yaml_value(node: &Yaml) -> Result<Value, String> {
    Ok(match node {
        Yaml::String(text) => Value::String(text.clone()),
        Yaml::Integer(n) => Value::Number((*n).into()),
        Yaml::Real(text) => node
            .as_f64()
            .and_then(Number::from_f64)
            .map_or_else(|| Value::String(text.clone()), Value::Number),
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Null => Value::Null,
        Yaml::Array(items) => Value::Array(items.iter().map(yaml_value).collect::<Result<_, _>>()?),
        Yaml::Hash(entries) => Value::Object(yaml_mapping(entries)?),
        Yaml::Alias(_) | Yaml::BadValue => {
            return Err("has a value that is not what its tag says, or an alias to no anchor".to_string());
        }
    })
}

/// A mapping as an object whose keys keep the file's order.
fn yaml_mapping(entries: &yaml_rust2::yaml::Hash) -> Result<Map<String, Value>, String> {
    entries.iter().map(|(key, value)| Ok((yaml_key(key)?, yaml_value(value)?))).collect()
}

/// A mapping's key as the text of a JSON key: a scalar as YAML wrote it.
fn yaml_key(key: &Yaml) -> Result<String, String> {
    match key {
        Yaml::String(text) | Yaml::Real(text) => Ok(text.clone()),
        Yaml::Integer(n) => Ok(n.to_string()),
        Yaml::Boolean(b) => Ok(b.to_string()),
        Yaml::Null => Ok("null".to_string()),
        _ => Err("has a mapping key that is not a scalar".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_mapping_is_read_in_order_with_its_scalars_typed() {
        let text =
            "packages:\n  - apps/*\n  - libs/*\nretries: 3\nratio: 0.5\nsticky: true\nnone: ~\nz: 1\na: 2\n";
        let data = parse_yaml(text).unwrap();
        assert_eq!(
            data,
            json!({ "packages": ["apps/*", "libs/*"], "retries": 3, "ratio": 0.5, "sticky": true, "none": null, "z": 1, "a": 2 })
        );
        let keys: Vec<&String> = data.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["packages", "retries", "ratio", "sticky", "none", "z", "a"]);
    }

    #[test]
    fn an_alias_stands_for_its_anchor_and_keys_are_text() {
        let text =
            ".base: &base\n  image: node:22\ntest:\n  <<: *base\n  script: [npm test]\n1: one\nlimit: .inf\n";
        assert_eq!(
            parse_yaml(text).unwrap(),
            json!({
                ".base": { "image": "node:22" },
                "test": { "<<": { "image": "node:22" }, "script": ["npm test"] },
                "1": "one",
                "limit": ".inf",
            })
        );
    }

    #[test]
    fn what_is_not_one_mapping_is_refused() {
        assert_eq!(parse_yaml("- a\n- b\n"), Err("not a YAML mapping".to_string()));
        assert_eq!(parse_yaml(""), Err("not a YAML mapping".to_string()));
        assert_eq!(parse_yaml("a: 1\n---\nb: 2\n"), Err("holds more than one YAML document".to_string()));
        assert_eq!(
            parse_yaml("a: 1\nb: [\n"),
            Err("not valid YAML: while parsing a node, did not find expected node content (line 3)"
                .to_string())
        );
        assert_eq!(parse_yaml("? [a]\n: 1\n"), Err("has a mapping key that is not a scalar".to_string()));
        assert_eq!(
            parse_yaml("n: !!int abc\n"),
            Err("has a value that is not what its tag says, or an alias to no anchor".to_string())
        );
    }
}
