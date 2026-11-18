//! The values of a data block, shown as Rust shows a fact's value of the same value.

use serde_json::Value;

use crate::harness::json::NULL_SHOWN;

/// A value as Rust displays what it reads that JSON as: a number written as an integer as
/// that integer, any other as an `f64` (`1.0` is `1`), a string as its text, `null` as the
/// `Option::None` it stands for, as a manifest's JSON value shows it too, and `true` and
/// `false` as themselves.
pub(crate) fn shown(value: &Value) -> String {
    match value {
        Value::Null => NULL_SHOWN.into(),
        Value::Number(number) => match number.as_f64().filter(|_| number.is_f64()) {
            Some(float) => float.to_string(),
            None => number.to_string(),
        },
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::shown;
    use serde_json::json;

    #[test]
    fn a_data_blocks_values_show_as_rust_shows_them() {
        let values = json!([true, null, 3, 18446744073709551615u64, 1.0, 1.5, 1e21, "a b"]);
        let shown: Vec<String> = values.as_array().unwrap().iter().map(shown).collect();
        let want = ["true", "None", "3", "18446744073709551615", "1", "1.5", "1000000000000000000000", "a b"];
        assert_eq!(shown, want);
    }
}
