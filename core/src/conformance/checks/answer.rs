//! Reading an answer the way the checks look at it. Every answer is also held to the schemas,
//! so a value of another shape reads here as empty and fails there.

use serde_json::Value;

/// The strings of an array; none when it is not one.
pub fn texts(value: &Value) -> Vec<&str> {
    items(value).iter().filter_map(Value::as_str).collect()
}

/// The items of an array; none when it is not one.
pub fn items(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// The keys of an object, sorted; none when it is not one.
pub fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> =
        value.as_object().map(|object| object.keys().map(String::as_str).collect()).unwrap_or_default();
    keys.sort_unstable();
    keys
}
