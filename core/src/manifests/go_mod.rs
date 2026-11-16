//! A `go.mod` as data: the directives a page states facts about, each the one argument it
//! was given, by its name: `module`, `go` and `toolchain`. Other directives, and the blocks
//! (`require ( … )`) they are written in, are not read.

use serde_json::{Map, Value};

/// The directives read, by the name `go.mod` gives them.
const READ: [&str; 3] = ["module", "go", "toolchain"];

/// The directives of a `go.mod` that are read; or why it is not one, as a phrase.
pub(super) fn parse_go_mod(text: &str) -> Result<Value, String> {
    let mut data = Map::new();
    let mut in_block = false;
    for (index, line) in text.lines().enumerate() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if in_block {
            in_block = line != ")";
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(directive) = words.next() else { continue };
        let arguments: Vec<&str> = words.collect();
        if arguments.last() == Some(&"(") {
            in_block = true;
            continue;
        }
        if !READ.contains(&directive) {
            continue;
        }
        let [argument] = arguments[..] else {
            return Err(format!("not a valid go.mod: line {}: {directive} takes one argument", index + 1));
        };
        data.insert(directive.to_string(), Value::String(unquoted(argument).to_string()));
    }
    Ok(Value::Object(data))
}

/// A module path may be written as a Go string: `"example.com/m"` or `` `example.com/m` ``.
fn unquoted(argument: &str) -> &str {
    ['"', '`']
        .iter()
        .find_map(|quote| argument.strip_prefix(*quote)?.strip_suffix(*quote))
        .unwrap_or(argument)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn module_go_and_toolchain_are_read_and_blocks_are_passed_over() {
        let text = "// the module\nmodule \"example.com/m\" // its path\n\ngo 1.23\n\ntoolchain go1.24.1\n\nrequire (\n\tgo v1.0.0\n)\n\nrequire example.com/x v1.2.3\n";
        assert_eq!(
            parse_go_mod(text),
            Ok(json!({ "module": "example.com/m", "go": "1.23", "toolchain": "go1.24.1" }))
        );
        assert_eq!(parse_go_mod("module m\n"), Ok(json!({ "module": "m" })));
    }

    #[test]
    fn a_directive_read_needs_its_one_argument() {
        assert_eq!(
            parse_go_mod("module m\ngo\n"),
            Err("not a valid go.mod: line 2: go takes one argument".to_string())
        );
    }
}
