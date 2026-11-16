//! JSON as docattest reads and writes it: parsing with a message for the user, and the
//! indented text of files and `--json` output.

use serde_json::Value;

/// The value `text` holds, or why it is not JSON.
pub(crate) fn parse_json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// `text` as a JSON string literal: quoted, so a message shows its spaces and escapes.
pub(crate) fn json_string(text: &str) -> String {
    Value::String(text.to_string()).to_string()
}

/// The value as indented text, two spaces a level, keys in their order.
pub(crate) fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("a JSON value always serialises")
}
