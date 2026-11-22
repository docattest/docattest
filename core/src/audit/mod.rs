//! `audit` and `inventory`: literals on living pages that look copied from a surface and carry no
//! directive, and what the surfaces offer.
//!
//! `candidates` finds lists, table columns and fenced blocks no directive binds, and `prose`
//! the sentences; `inventory` indexes what the adapter's `observables` offer; `matching`
//! holds the thresholds; `suggest` picks the best value for a list, table or block, and
//! `inline` for a sentence or a single value, or stays silent; `run` drives the adapter and
//! evaluates every suggestion again; `text` and `listing` print.

pub mod apply;
pub mod candidates;
pub mod inline;
pub mod inventory;
pub mod listing;
pub mod matching;
pub mod prose;
pub mod run;
pub mod suggest;
pub mod text;

use serde_json::{Map, Value, json};

pub use crate::compare::Observation;

/// What a suggestion's literal is: a list, a table column, a fenced block, an enumeration
/// written inline in a sentence (its code spans), or a single value in prose.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    List,
    Column,
    Block,
    Inline,
    /// A code span or a bare number, as the page writes it (`` `PORT` ``, `8080`): the
    /// directive goes right after it, so that it binds that value alone.
    Scalar {
        after: String,
    },
}

impl Target {
    pub fn as_str(&self) -> &'static str {
        match self {
            Target::List => "list",
            Target::Column => "column",
            Target::Block => "block",
            Target::Inline => "inline",
            Target::Scalar { .. } => "scalar",
        }
    }
}

/// `match` when page and code agree, `drift` when they nearly agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Match,
    Drift,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Match => "match",
            Status::Drift => "drift",
        }
    }
}

/// One value a surface offers, with the fact expression that reads it (the adapter's
/// `Observable`; its `path` is not used here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observable {
    pub surface: String,
    pub expression: String,
    pub value: Observation,
}

/// A literal on a living page that looks copied from a surface and carries no directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub page: String,
    /// The first line of the list, table or block; for an inline enumeration, the line its
    /// sentence ends on; for a scalar, its line.
    pub line: usize,
    pub target: Target,
    /// A fact expression over the surface that the literal matches or nearly matches.
    pub expression: String,
    pub status: Status,
    /// In the code, not on the page.
    pub missing: Vec<String>,
    /// On the page, not in the code.
    pub unexpected: Vec<String>,
    /// The surface the expression reads.
    pub surface: String,
    /// The directive to paste above the list, table or block, at the end of the sentence or
    /// right after the scalar.
    pub directive: String,
}

impl Suggestion {
    /// The JSON form, keys in the order the recordings hold.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("page".into(), json!(self.page));
        m.insert("line".into(), json!(self.line));
        m.insert("target".into(), json!(self.target.as_str()));
        m.insert("expression".into(), json!(self.expression));
        m.insert("status".into(), json!(self.status.as_str()));
        m.insert("missing".into(), json!(self.missing));
        m.insert("unexpected".into(), json!(self.unexpected));
        m.insert("surface".into(), json!(self.surface));
        m.insert("directive".into(), json!(self.directive));
        if let Target::Scalar { after } = &self.target {
            m.insert("after".into(), json!(after));
        }
        Value::Object(m)
    }
}

/// Everything `audit` found, as `--json` prints it.
pub fn report_json(suggestions: &[Suggestion]) -> Value {
    json!({ "suggestions": suggestions.iter().map(Suggestion::to_json).collect::<Vec<_>>() })
}

/// One value a surface offers, with the fact expression that reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    pub surface: String,
    /// A fact expression that evaluates to this value.
    pub expression: String,
    /// `String(value)`.
    pub value: String,
    /// Each item, when the value is a list.
    pub items: Option<Vec<String>>,
}

impl InventoryEntry {
    /// The JSON form, keys in the order the recordings hold.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("surface".into(), json!(self.surface));
        m.insert("expression".into(), json!(self.expression));
        m.insert("value".into(), json!(self.value));
        if let Some(items) = &self.items {
            m.insert("items".into(), json!(items));
        }
        Value::Object(m)
    }
}

/// Distinct strings in first-seen order.
pub(crate) fn distinct<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut seen = std::collections::HashSet::new();
    items.into_iter().filter(|item| seen.insert(*item)).collect()
}
