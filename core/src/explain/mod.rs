//! `explain`: what each directive on a page binds and why, the sentence it defends, and what
//! checking it gives (`src/explain/`). The planner decides what a directive binds; this only
//! says what it decided ([`crate::attestations::plan::Why`]), beside the check's results.

mod text;

use serde_json::{Map, Value, json};

use crate::api::attestation::{Attestation, Body};
use crate::attestations::alias::Alias;
use crate::attestations::bind::FactRule;
use crate::attestations::placement::Placed;
use crate::attestations::plan::Why;
use crate::report::Outcome;
pub use text::format;

/// One directive explained.
#[derive(Debug, Clone, PartialEq)]
pub enum Explanation {
    /// A directive that attests: its attestation as planned, why it is what it is, and the
    /// results checking it gives.
    Attests { attestation: Box<Attestation>, why: Why, results: Vec<Outcome> },
    /// A test file alias: the lines of the citations on the page that use it.
    Alias { alias: Alias, line: usize, cited_at: Vec<usize> },
}

impl Explanation {
    /// An alias declared on `line`, with the lines of the citations among `page` that name
    /// it.
    pub fn of_alias(alias: &Alias, line: usize, page: &[&Attestation]) -> Explanation {
        let names_it =
            |a: &&&Attestation| matches!(&a.body, Body::Test(t) if t.alias.as_ref() == Some(&alias.name));
        let mut cited_at: Vec<usize> = page.iter().filter(names_it).map(|a| a.line).collect();
        cited_at.dedup();
        Explanation::Alias { alias: alias.clone(), line, cited_at }
    }

    pub fn line(&self) -> usize {
        match self {
            Explanation::Attests { attestation, .. } => attestation.line,
            Explanation::Alias { line, .. } => *line,
        }
    }
}

/// The explanations of a page as `explain --json` prints them.
pub fn to_json(page: &str, explanations: &[Explanation]) -> Value {
    json!({ "page": page, "directives": explanations.iter().map(explanation_json).collect::<Vec<_>>() })
}

fn explanation_json(explanation: &Explanation) -> Value {
    match explanation {
        Explanation::Attests { attestation, why, results } => {
            let mut m = Map::new();
            m.insert("line".into(), json!(attestation.line));
            m.insert("kind".into(), json!(attestation.kind().as_str()));
            m.insert("placed".into(), placed_json(why.placed));
            if let Some(rule) = why.rule {
                m.insert("rule".into(), json!(rule_name(rule)));
            }
            if let Body::Test(test) = &attestation.body
                && let Some(alias) = &test.alias
            {
                m.insert("alias".into(), json!(alias));
            }
            m.insert("attestation".into(), attestation.to_json());
            m.insert("results".into(), results.iter().map(Outcome::to_json).collect());
            Value::Object(m)
        }
        Explanation::Alias { alias, line, cited_at } => json!({
            "line": line, "kind": "alias", "name": alias.name, "file": alias.file, "citedAt": cited_at,
        }),
    }
}

fn placed_json(placed: Placed) -> Value {
    match placed {
        Placed::Written => json!({ "where": "written" }),
        Placed::Above { line } => json!({ "where": "above", "line": line }),
        Placed::Detached => json!({ "where": "detached" }),
    }
}

/// A rule's name in the JSON form.
fn rule_name(rule: FactRule) -> &'static str {
    match rule {
        FactRule::Field => "field",
        FactRule::Column => "column",
        FactRule::BlockBelow => "block-below",
        FactRule::CodeSpan => "code-span",
        FactRule::Literal => "literal",
        FactRule::SentenceSpans => "sentence-spans",
        FactRule::SentenceSpan => "sentence-span",
        FactRule::BlockAfter => "block-after",
    }
}

#[cfg(test)]
mod tests;
