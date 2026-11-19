//! What `explain` prints: for each directive, its line and words, then one labelled line for
//! each thing said of it.

use super::Explanation;
use crate::api::attestation::{Attestation, Body, Cases, Fact, FactTarget, Test};
use crate::api::field::as_json_writes;
use crate::attestations::bind::FactRule;
use crate::attestations::placement::Placed;
use crate::attestations::plan::Why;
use crate::counted::counted;
use crate::report::{Outcome, status_mark};

/// How wide the labels are, so the values line up.
const LABEL: usize = 10;

/// The explanations of a page, one block of lines each, parted by blank lines.
pub fn format(page: &str, explanations: &[Explanation]) -> String {
    let blocks: Vec<String> = explanations.iter().map(|e| explanation(page, e).join("\n")).collect();
    blocks.join("\n\n")
}

fn explanation(page: &str, explanation: &Explanation) -> Vec<String> {
    match explanation {
        Explanation::Attests { attestation, why, results } => attests(page, attestation, *why, results),
        Explanation::Alias { alias, line, cited_at } => {
            let cited = cited_at.iter().map(|line| format!("L{line}")).collect::<Vec<_>>().join(", ");
            let cited = if cited.is_empty() { "no citation on the page".to_string() } else { cited };
            vec![
                format!("{page}:{line} tests: {} = {}", alias.name, alias.file),
                labelled("cited by", &cited),
            ]
        }
    }
}

fn attests(page: &str, a: &Attestation, why: Why, results: &[Outcome]) -> Vec<String> {
    let mut lines = vec![format!("{page}:{} {}", a.line, heading(&a.body))];
    lines.push(labelled("claim", if a.claim.is_empty() { "none" } else { &a.claim }));
    lines.push(labelled("placed", &placed(why.placed)));
    match &a.body {
        Body::Fact(fact) => lines.extend(fact_lines(fact, why.rule)),
        Body::Test(test) if !test.file.is_empty() => lines.push(labelled("file", &cited_file(test))),
        Body::Cases(cases) if a.ok() => lines.push(labelled("binds", &table(cases))),
        _ => {}
    }
    lines.extend(result_lines(a, results));
    lines
}

/// The directive's kind and words, as the page writes them, with an alias's file in its
/// place; the kind alone for a directive too malformed to have words.
fn heading(body: &Body) -> String {
    let (kind, words) = match body {
        Body::Fact(fact) => ("fact", fact.expression.clone()),
        Body::Test(test) if test.file.is_empty() => ("test", String::new()),
        Body::Test(test) => {
            let titles = test.describes.iter().chain(std::iter::once(&test.title));
            let path: Vec<String> = titles.map(|t| format!("\"{t}\"")).collect();
            ("test", format!("{} > {}", test.file, path.join(" > ")))
        }
        Body::Cases(cases) => ("cases", cases.runner.clone()),
        Body::Example(_) | Body::Console(_) | Body::Sample(_) => ("example", String::new()),
    };
    if words.is_empty() { kind.to_string() } else { format!("{kind}: {words}") }
}

fn placed(placed: Placed) -> String {
    match placed {
        Placed::Written => "where it is written".into(),
        Placed::Above { line } => {
            format!("on the sentence below it, as if written where that sentence ends, on L{line}")
        }
        Placed::Detached => "nowhere: a blank line parts it from the prose below".into(),
    }
}

fn fact_lines(fact: &Fact, rule: Option<FactRule>) -> Vec<String> {
    let mut lines = Vec::new();
    if let (Some(rule), Ok(target)) = (rule, &fact.target) {
        lines.push(labelled("binds", &rule_sentence(rule, fact)));
        lines.extend(target_lines(target));
    }
    let ordered = if fact.ordered { ", in order" } else { "" };
    lines.push(labelled("surface", &format!("{}{ordered}", fact.surface)));
    lines
}

/// What a rule binds, in words.
fn rule_sentence(rule: FactRule, fact: &Fact) -> String {
    let named = |name: &Option<String>| name.clone().unwrap_or_default();
    match rule {
        FactRule::Field => format!("field={}: that value of the fenced block after it", named(&fact.field)),
        FactRule::Column => format!("column={}: that column of the table after it", named(&fact.column)),
        FactRule::BlockBelow => "the list or fenced block on the line after it".into(),
        FactRule::CodeSpan => "the code span before it on its line".into(),
        FactRule::Literal => "the number or version before it on its line".into(),
        FactRule::SentenceSpans => "the code spans of the sentence it ends, as a set".into(),
        FactRule::SentenceSpan => "the code span of the sentence it ends, on an earlier line".into(),
        FactRule::BlockAfter => "the list or fenced block after the paragraph it ends".into(),
    }
}

/// The target as the page has it: a value, a list's items, or a block's lines.
fn target_lines(target: &FactTarget) -> Vec<String> {
    match target {
        FactTarget::Scalar(value) => vec![labelled("target", value)],
        FactTarget::Set { items, .. } => {
            vec![labelled("target", &format!("{} ({})", items.join(", "), counted(items.len(), "item")))]
        }
        FactTarget::Text(text) => continued("target", text.lines()),
        FactTarget::Field { .. } => target_lines(&target.spelled(as_json_writes)),
    }
}

fn cited_file(test: &Test) -> String {
    match &test.alias {
        Some(alias) => format!("{}, by the alias {alias}", test.file),
        None => test.file.clone(),
    }
}

fn table(cases: &Cases) -> String {
    let rows = counted(cases.rows.len(), "row");
    format!("the table after it: {rows}, columns {}", cases.header.join(", "))
}

/// For a fact, the value its expression gives; then each result, with its line when it is a
/// row's.
fn result_lines(a: &Attestation, results: &[Outcome]) -> Vec<String> {
    let value = results.first().and_then(|r| r.actual.as_deref()).filter(|_| matches!(a.body, Body::Fact(_)));
    let shown = results.iter().map(|r| {
        let at = if r.line == a.line { String::new() } else { format!("L{} ", r.line) };
        format!("{} {at}{}: {}", status_mark(r.status), r.status.as_str(), r.message)
    });
    let mut lines: Vec<String> = value.map(|value| labelled("value", value)).into_iter().collect();
    lines.extend(continued("result", shown));
    lines
}

/// `label` and the first of `values`, then each of the others under it.
fn continued<S: AsRef<str>>(label: &str, values: impl Iterator<Item = S>) -> Vec<String> {
    let indent = " ".repeat(LABEL + 2);
    let line = |(k, value): (usize, S)| match k {
        0 => labelled(label, value.as_ref()),
        _ => format!("{indent}{}", value.as_ref()),
    };
    values.enumerate().map(line).collect()
}

fn labelled(label: &str, value: &str) -> String {
    format!("  {label:LABEL$}{value}")
}
