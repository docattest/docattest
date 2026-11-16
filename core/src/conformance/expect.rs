//! What a check states, and how it fails: each statement is a [`Checked`], and the first that
//! does not hold, in the order stated, ends the check with its message.

use serde_json::Value;

/// A step of a check: its value, or why the check fails.
pub type Checked<T = ()> = Result<T, String>;

/// `holds` is true; else the check fails, saying `what`.
pub fn ensure(holds: bool, what: impl Into<String>) -> Checked {
    if holds { Ok(()) } else { Err(what.into()) }
}

/// `actual` is `expected`; else the check fails, saying `what` and both values.
pub fn same(actual: &Value, expected: &Value, what: &str) -> Checked {
    ensure(actual == expected, format!("{what}: expected {expected}, got {actual}"))
}

/// Every statement holds; else the first that does not, in the order given.
pub fn all_hold(statements: impl IntoIterator<Item = Checked>) -> Checked {
    statements.into_iter().collect()
}
