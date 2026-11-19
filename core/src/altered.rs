//! A value changed into another of the same kind, as `mutate` changes what a proof reads
//! where the core itself reads it: a file a fact reads, a value of a manifest, a cell of a
//! table. Code is the adapter's to change (the `mutants` method); text and JSON are the
//! same in every language.

use serde_json::{Number, Value};

/// What `altered_text` appends to a text that holds no number.
const APPENDED: &str = "x";

/// `text` with its last run of ASCII digits counted up by one (`0.2.2` → `0.2.3`, `99` →
/// `100`), a number being what a text most often pins; else with `x` appended.
pub fn altered_text(text: &str) -> String {
    let Some(end) = text.rfind(|c: char| c.is_ascii_digit()).map(|k| k + 1) else {
        return format!("{text}{APPENDED}");
    };
    let start = text[..end].trim_end_matches(|c: char| c.is_ascii_digit()).len();
    format!("{}{}{}", &text[..start], counted_up(&text[start..end]), &text[end..])
}

/// A run of decimal digits plus one, as long as it needs to be (`099` → `100`).
fn counted_up(digits: &str) -> String {
    let mut out: Vec<u8> = digits.bytes().collect();
    for digit in out.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return String::from_utf8(out).expect("ASCII digits are UTF-8");
        }
    }
    format!("1{}", String::from_utf8(out).expect("ASCII digits are UTF-8"))
}

/// A cell of a table changed: the content of its last code span, which is what a runner
/// most often reads of a cell that has one; else the cell's text. Backticks are never
/// added, so the span stays one span.
pub fn altered_cell(cell: &str) -> String {
    match last_code_span(cell) {
        Some((start, end)) => {
            format!("{}{}{}", &cell[..start], altered_text(&cell[start..end]), &cell[end..])
        }
        None => altered_text(cell),
    }
}

/// Where the content of the last code span of `text` starts and ends: a run of backticks
/// opens a span that the next run of the same length closes.
fn last_code_span(text: &str) -> Option<(usize, usize)> {
    let run_at = |at: usize| text[at..].bytes().take_while(|b| *b == b'`').count();
    let mut last = None;
    let mut at = 0;
    while let Some(open) = text[at..].find('`').map(|k| at + k) {
        let length = run_at(open);
        let content = open + length;
        let mut search = content;
        let close = loop {
            let Some(next) = text[search..].find('`').map(|k| search + k) else { break None };
            if run_at(next) == length {
                break Some(next);
            }
            search = next + run_at(next);
        };
        let Some(close) = close else {
            at = content;
            continue;
        };
        last = Some((content, close));
        at = close + length;
    }
    last
}

/// A scalar of JSON changed: a number plus one, a string as [`altered_text`] changes it,
/// a boolean flipped. `null`, a list and an object have no change of their own.
pub fn altered_scalar(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(b) => Some(Value::Bool(!b)),
        Value::String(s) => Some(Value::String(altered_text(s))),
        Value::Number(n) => plus_one(n).map(Value::Number),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// `n + 1`, an integer staying an integer.
fn plus_one(n: &Number) -> Option<Number> {
    if let Some(i) = n.as_i64() {
        return i.checked_add(1).map(Number::from);
    }
    if let Some(u) = n.as_u64() {
        return u.checked_add(1).map(Number::from);
    }
    n.as_f64().and_then(|f| Number::from_f64(f + 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_text_has_its_last_number_counted_up_or_gains_an_x() {
        assert_eq!(altered_text("10001"), "10002");
        assert_eq!(altered_text("0.2.2"), "0.2.3");
        assert_eq!(altered_text("v1.9 beta"), "v1.10 beta");
        assert_eq!(altered_text("099"), "100");
        assert_eq!(altered_text("FROM node:22 AS build"), "FROM node:23 AS build");
        assert_eq!(altered_text("shop"), "shopx");
        assert_eq!(altered_text(""), "x");
        assert_eq!(altered_text("é9"), "é10");
    }

    #[test]
    fn a_cell_has_its_last_code_span_changed_and_keeps_its_backticks() {
        assert_eq!(altered_cell("`FOO=1` `BAR=2`"), "`FOO=1` `BAR=3`");
        assert_eq!(altered_cell("``a`b``"), "``a`bx``");
        assert_eq!(altered_cell("`a` and 7"), "`ax` and 7");
        assert_eq!(altered_cell("rounds up"), "rounds upx");
        assert_eq!(altered_cell("an ` alone"), "an ` alonex");
    }

    #[test]
    fn a_scalar_changes_within_its_kind() {
        assert_eq!(altered_scalar(&json!(3)), Some(json!(4)));
        assert_eq!(altered_scalar(&json!(-1)), Some(json!(0)));
        assert_eq!(altered_scalar(&json!(1.5)), Some(json!(2.5)));
        assert_eq!(altered_scalar(&json!(true)), Some(json!(false)));
        assert_eq!(altered_scalar(&json!(">=22")), Some(json!(">=23")));
        assert_eq!(altered_scalar(&json!(null)), None);
        assert_eq!(altered_scalar(&json!([1])), None);
    }
}
