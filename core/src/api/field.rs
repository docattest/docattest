//! The value a `field=` fact reads from a data block, kept as the block holds it: the block is
//! written in its data format, not in the project's language, so its value is spelled the
//! language's way (the adapter's `display`) before it is compared.

use serde_json::Value;

/// A value of a data block that a fact compares with: a scalar (a string, number, boolean or
/// null), or a list of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Scalar(Value),
    List(Vec<Value>),
}

impl FieldValue {
    /// Its scalars, in order: the one, or the list's.
    pub fn scalars(&self) -> &[Value] {
        match self {
            FieldValue::Scalar(scalar) => std::slice::from_ref(scalar),
            FieldValue::List(items) => items,
        }
    }
}

/// A scalar of a data block as JSON writes it, a string as its text: `true`, `null`, `1.0`.
/// A field is compared so when the adapter does not display values.
pub fn as_json_writes(scalar: &Value) -> String {
    match scalar {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_writes_a_string_as_its_text_and_the_rest_as_json() {
        let written: Vec<String> =
            [json!("a b"), json!(true), json!(null), json!(1.0), json!(3), json!(1e21)]
                .iter()
                .map(as_json_writes)
                .collect();
        assert_eq!(written, ["a b", "true", "null", "1.0", "3", "1e+21"]);
    }
}
