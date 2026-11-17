//! Comparing an observed value with a fact's target.

mod block;

use std::collections::HashSet;

use crate::api::attestation::{FactTarget, SetFrom};
use crate::attestations::literals::without_version_mark;
use crate::counted::{agreeing, counted};
use crate::json::json_string;
use crate::report::Status;
use crate::text::{collapse_spaces, compare_utf16, trim};
use block::first_difference;

/// A fact's value as the adapter observed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// The value as the language itself displays it (PROTOCOL.md §2).
    pub text: String,
    /// Each item as a string, when the value is a list, a set or another iterable.
    pub items: Option<Vec<String>>,
    /// A short description for messages, such as `an object` or `number 3`.
    pub description: String,
    /// Whether the value is text, a string of the language (`"kind": "text"`): the one thing
    /// about a value's type the core needs, since only text can be what a block copies.
    pub is_text: bool,
}

/// The verdict of one fact comparison. Messages always carry both sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub status: Status,
    pub message: String,
    pub expected: String,
    pub actual: String,
}

fn list<S: std::borrow::Borrow<str>>(items: &[S]) -> String {
    format!("[{}]", items.join(", "))
}

fn collapse(text: &str) -> String {
    trim(&collapse_spaces(text)).to_string()
}

/// Compare a fact's observed value with its target: scalars as strings, sets
/// order-insensitive unless `ordered`, text token by token (whitespace counts only where it
/// parts two words; see [`first_difference`]). A number or version
/// compares without the `v` that may lead it, so `v1.2.3` is `1.2.3`.
pub fn compare_fact(expression: &str, ordered: bool, target: &FactTarget, value: &Observation) -> Comparison {
    match target {
        FactTarget::Scalar(expected) => compare_scalar(expression, expected, value),
        FactTarget::Text(page) => compare_text(expression, page, value),
        FactTarget::Set { items, from } => compare_set(expression, ordered, Expected { items, from }, value),
    }
}

fn compare_scalar(expression: &str, expected: &str, value: &Observation) -> Comparison {
    let ok = without_version_mark(&value.text) == without_version_mark(expected);
    let message = if ok {
        format!("{expression} is {}", value.text)
    } else {
        format!("expected {expected}, got {} from {expression}", value.text)
    };
    verdict(ok, message, expected.to_string(), value.text.clone())
}

fn compare_text(expression: &str, page: &str, value: &Observation) -> Comparison {
    let (expected, actual) = (collapse(page), collapse(&value.text));
    let Some(difference) = first_difference(page, &value.text) else {
        return verdict(true, format!("{expression} matches the block"), expected, actual);
    };
    let shown =
        |side: Option<String>, end: &str| side.map_or_else(|| end.to_string(), |text| json_string(&text));
    let message = format!(
        "the block differs from {expression} at line {} (whitespace ignored): expected {}, got {}",
        difference.line,
        shown(difference.expected, "the end of the block"),
        shown(difference.actual, "the end of the value"),
    );
    verdict(false, message, expected, actual)
}

fn verdict(ok: bool, message: String, expected: String, actual: String) -> Comparison {
    Comparison { status: if ok { Status::Pass } else { Status::Fail }, message, expected, actual }
}

/// The set a fact expects: its items, and where on the page they are.
struct Expected<'a> {
    items: &'a [String],
    from: &'a SetFrom,
}

/// How a set's values are counted in a message: each one, or each value once when one was
/// written twice.
#[derive(Clone, Copy)]
enum Counting {
    Each,
    Distinct,
}

impl Expected<'_> {
    /// `count` of what the items are: `1 item`, `3 values in column path`, `2 distinct code
    /// spans`.
    fn counted(&self, count: usize, counting: Counting) -> String {
        let distinct = matches!(counting, Counting::Distinct);
        let (noun, place) = match self.from {
            SetFrom::Table { column } => {
                (if distinct { "distinct value" } else { "value" }, format!(" in column {column}"))
            }
            SetFrom::List => (if distinct { "distinct item" } else { "item" }, String::new()),
            SetFrom::Sentence => (if distinct { "distinct code span" } else { "code span" }, String::new()),
        };
        format!("{}{place}", counted(count, noun))
    }

    /// Where the items are on the page: `the list`, `column path`.
    fn place(&self) -> String {
        match self.from {
            SetFrom::Table { column } => format!("column {column}"),
            SetFrom::List | SetFrom::Sentence => format!("the {}", self.from.as_str()),
        }
    }
}

fn compare_set(expression: &str, ordered: bool, expected: Expected, value: &Observation) -> Comparison {
    let Some(items) = &value.items else {
        if let (SetFrom::Sentence, false, Some(last)) = (expected.from, ordered, expected.items.last()) {
            return compare_scalar(expression, last, value);
        }
        let what =
            format!("{expression} must be a list to compare with the {} on the page", expected.from.as_str());
        return Comparison {
            status: Status::Error,
            message: format!("{what}, got {}", value.description),
            expected: list(expected.items),
            actual: value.description.clone(),
        };
    };
    let (page_items, code_items) = (keyed(expected.items), keyed(items));
    let problems = set_problems(&page_items, &code_items, ordered);
    // Unordered, the two sides are sets, so a value written twice counts once.
    let count = |values: &[SetItem]| if ordered { values.len() } else { unique(values).count() };
    let (want, have) = (count(&page_items), count(&code_items));
    let repeats = want != page_items.len() || have != code_items.len();
    let counting = if repeats { Counting::Distinct } else { Counting::Each };
    // What differs leads; the counts follow only when they differ, as equal counts beside
    // a difference read as a contradiction.
    let message = if problems.is_empty() {
        let verb = agreeing(want, "matches", "match");
        format!("{} {verb} {expression}", expected.counted(want, counting))
    } else if want == have {
        format!("{} (comparing {} on the page with {expression})", problems.join("; "), expected.place())
    } else {
        let counts = format!("{} on the page, {have} in {expression}", expected.counted(want, counting));
        format!("{} ({counts})", problems.join("; "))
    };
    Comparison {
        status: if problems.is_empty() { Status::Pass } else { Status::Fail },
        message,
        expected: list(expected.items),
        actual: list(items),
    }
}

/// An item of a set as written, with the key it compares by: without the `v` that may lead
/// a number or version.
struct SetItem<'a> {
    written: &'a str,
    key: &'a str,
}

fn keyed(items: &[String]) -> Vec<SetItem<'_>> {
    items.iter().map(|item| SetItem { written: item, key: without_version_mark(item) }).collect()
}

/// Unique items by key, in first-seen order: `new Set(values)`.
fn unique<'a>(values: &'a [SetItem<'a>]) -> impl Iterator<Item = &'a SetItem<'a>> {
    let mut seen = HashSet::new();
    values.iter().filter(move |v| seen.insert(v.key))
}

/// An item as a message names it: an empty one, such as an empty cell, as `""`.
fn shown(item: &str) -> &str {
    if item.is_empty() { "\"\"" } else { item }
}

fn set_problems(expected: &[SetItem], actual: &[SetItem], ordered: bool) -> Vec<String> {
    let (missing, unexpected) = (absent_from(actual, expected), absent_from(expected, actual));
    let join = |v: &[&str]| v.iter().map(|s| shown(s)).collect::<Vec<_>>().join(", ");
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("missing: {}", join(&missing)));
    }
    if !unexpected.is_empty() {
        problems.push(format!("unexpected: {}", join(&unexpected)));
    }
    let reordered = expected.iter().map(|v| v.key).ne(actual.iter().map(|v| v.key));
    if ordered && problems.is_empty() && reordered {
        let written: Vec<&str> = actual.iter().map(|v| v.written).collect();
        problems.push(format!("order differs: the code has {}", list(&written)));
    }
    problems
}

/// The unique `items` whose key `other` lacks, as written, in UTF-16 order.
fn absent_from<'a>(items: &'a [SetItem<'a>], other: &[SetItem]) -> Vec<&'a str> {
    let keys: HashSet<&str> = other.iter().map(|v| v.key).collect();
    let mut absent: Vec<&str> = unique(items).filter(|x| !keys.contains(x.key)).map(|x| x.written).collect();
    absent.sort_by(|a, b| compare_utf16(a, b));
    absent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(text: &str, items: Option<&[&str]>) -> Observation {
        Observation {
            text: text.into(),
            items: items.map(|i| i.iter().map(|s| s.to_string()).collect()),
            description: "an object".into(),
            is_text: false,
        }
    }

    #[test]
    fn scalars_compare_as_strings() {
        let c = compare_fact("N", false, &FactTarget::Scalar("3".into()), &obs("3", None));
        assert_eq!((c.status, c.message.as_str()), (Status::Pass, "N is 3"));
        let c = compare_fact("N", false, &FactTarget::Scalar("3".into()), &obs("4", None));
        assert_eq!(c.message, "expected 3, got 4 from N");
    }

    #[test]
    fn a_leading_v_is_not_part_of_a_number() {
        let c = compare_fact("V", false, &FactTarget::Scalar("v1.89.0".into()), &obs("1.89.0", None));
        assert_eq!((c.status, c.message.as_str()), (Status::Pass, "V is 1.89.0"));
        let c = compare_fact("V", false, &FactTarget::Scalar("1.89.0".into()), &obs("v1.89.0", None));
        assert_eq!(c.status, Status::Pass);
        let c = compare_fact("V", false, &FactTarget::Scalar("v1.89.0".into()), &obs("1.90.0", None));
        assert_eq!(c.message, "expected v1.89.0, got 1.90.0 from V");
        let list = FactTarget::Set { items: vec!["v1.0.0".into(), "v2.0.0".into()], from: SetFrom::List };
        let c = compare_fact("V", true, &list, &obs("", Some(&["1.0.0", "2.0.0"])));
        assert_eq!(c.status, Status::Pass);
    }

    #[test]
    fn sets_name_what_is_missing() {
        let target = FactTarget::Set {
            items: vec!["API_TOKEN".into(), "REGION".into(), "PORT".into()],
            from: SetFrom::List,
        };
        let c = compare_fact(
            "ENV_KEYS",
            false,
            &target,
            &obs("", Some(&["API_TOKEN", "REGION", "PORT", "LOG_LEVEL"])),
        );
        assert_eq!(c.message, "missing: LOG_LEVEL (3 items on the page, 4 in ENV_KEYS)");
        let c = compare_fact(
            "K",
            true,
            &FactTarget::Set { items: vec!["a".into(), "b".into()], from: SetFrom::List },
            &obs("", Some(&["b", "a"])),
        );
        assert_eq!(c.message, "order differs: the code has [b, a] (comparing the list on the page with K)");
        let c = compare_fact(
            "K",
            false,
            &FactTarget::Set { items: vec![], from: SetFrom::Table { column: "Field".into() } },
            &obs("x", None),
        );
        assert_eq!(c.message, "K must be a list to compare with the table on the page, got an object");
    }

    #[test]
    fn text_collapses_whitespace() {
        let c =
            compare_fact("S", false, &FactTarget::Text("type X =\n  1;".into()), &obs("type X = 1;", None));
        assert_eq!(c.status, Status::Pass);
        let c = compare_fact("S", false, &FactTarget::Text("x\na\"b".into()), &obs("x c", None));
        assert_eq!(
            c.message,
            "the block differs from S at line 2 (whitespace ignored): expected \"a\\\"b\", got \"c\""
        );
        assert_eq!((c.expected.as_str(), c.actual.as_str()), ("x a\"b", "x c"));
        let c = compare_fact("S", false, &FactTarget::Text("x".into()), &obs("x y", None));
        assert_eq!(
            c.message,
            "the block differs from S at line 2 (whitespace ignored): expected the end of the block, got \"y\""
        );
    }

    #[test]
    fn a_sentence_is_a_set_for_a_list_and_its_last_code_span_for_a_scalar() {
        let sentence = FactTarget::Set { items: vec!["A".into(), "B".into()], from: SetFrom::Sentence };
        let c = compare_fact("K", false, &sentence, &obs("", Some(&["A", "B", "C"])));
        assert_eq!(c.message, "missing: C (2 code spans on the page, 3 in K)");
        let c = compare_fact("N", false, &sentence, &obs("B", None));
        assert_eq!((c.status, c.message.as_str()), (Status::Pass, "N is B"));
        let c = compare_fact("N", true, &sentence, &obs("B", None));
        assert_eq!(c.message, "N must be a list to compare with the sentence on the page, got an object");
    }

    #[test]
    fn repeated_values_count_once_and_empty_ones_show() {
        let column = |items: &[&str]| FactTarget::Set {
            items: items.iter().map(|s| s.to_string()).collect(),
            from: SetFrom::Table { column: "Kind".into() },
        };
        let c = compare_fact("K", false, &column(&["a", "a", "b"]), &obs("", Some(&["a", "b"])));
        assert_eq!(
            (c.status, c.message.as_str()),
            (Status::Pass, "2 distinct values in column Kind match K")
        );
        let c = compare_fact("K", false, &column(&["a", "a", ""]), &obs("", Some(&["a", "b"])));
        assert_eq!(c.message, "missing: b; unexpected: \"\" (comparing column Kind on the page with K)");
        let c = compare_fact("K", false, &column(&["a", "a"]), &obs("", Some(&["a", "b"])));
        assert_eq!(c.message, "missing: b (1 distinct value in column Kind on the page, 2 in K)");
        let c = compare_fact("K", true, &column(&["a", "a"]), &obs("", Some(&["a"])));
        assert_eq!(
            c.message,
            "order differs: the code has [a] (2 values in column Kind on the page, 1 in K)"
        );
    }
}
