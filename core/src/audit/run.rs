//! Driving the adapter for `audit`: what the surfaces offer, the suggestions, and each suggested
//! expression evaluated again as `check` will evaluate it.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use super::candidates::{PageCandidates, candidates_of_page};
use super::examples::{collect_results, plan_examples, send_compile};
use super::inventory::Inventory;
use super::suggest::suggest_for_page;
use super::{Observable, Observation, Suggestion};
use crate::adapter::Pending;
use crate::api::attestation::Attestation;
use crate::api::errors::{Error, Result};
use crate::config::DEFAULT_SURFACE;
use crate::files::project_files::ProjectFiles;
use crate::project::check::Network;
use crate::project::check::open;
use crate::project::state::State;
use crate::report::order::by_page_and_line;

/// Find lists, tables and blocks on living pages that look copied from a surface and carry
/// no directive. Advisory and read-only: nothing is written and nothing on the pages is run.
/// A surface that cannot be loaded is an error, not an empty audit.
pub fn run_audit(state: &State, network: Network) -> Result<Vec<Suggestion>> {
    let pages = candidates(state)?;
    crate::trace::mark("candidates found");
    let planned_examples = plan_examples(state, &pages);
    let example_refs: Vec<&Attestation> = planned_examples.iter().map(|p| &p.attestation).collect();
    let opened = open(state, network, &example_refs);
    let offered = request_values(state);
    let compiled = send_compile(state, &planned_examples);
    opened.wait()?;
    let observables = offered.wait()?;
    let inventory = Inventory::new(&observables);
    let suggested: Vec<Suggestion> =
        pages.iter().flat_map(|(page, found)| suggest_for_page(page, found, &inventory)).collect();
    crate::trace::mark("suggested");
    let mut suggestions = verified(state, suggested, &observables)?;
    suggestions.extend(collect_results(&planned_examples, compiled)?);
    suggestions.sort_by(|a, b| by_page_and_line((&a.page, a.line), (&b.page, b.line)));
    Ok(suggestions)
}

/// The candidates of every living page that `pages` matches (not the source files whose
/// doc comments made them living), in page order.
fn candidates(state: &State) -> Result<Vec<(&String, PageCandidates)>> {
    let is_page = crate::config::matcher(&state.config.pages)?;
    let pages: Vec<&String> = state.living.iter().filter(|page| is_page.is_match(page)).collect();
    let files = &state.files;
    let find = |page: &&String| -> Result<PageCandidates> {
        let text = files.read(page).map_err(Error::user)?;
        candidates_of_page(page, &text).map_err(|why| why.on(page))
    };
    let found = crate::parallel::map_in_parallel(&pages, find);
    pages.into_iter().zip(found).map(|(page, found)| Ok((page, found?))).collect()
}

/// `observables` requests in flight, one per surface.
pub struct Offered {
    pending: Vec<Pending>,
}

/// Ask what every surface offers, all at once. The default surface first, so that of two
/// equal values the default one is suggested. An adapter without the `observables`
/// capability offers nothing.
pub fn request_values(state: &State) -> Offered {
    if !can_observe(state) {
        return Offered { pending: Vec::new() };
    }
    let names = state.config.surfaces.iter().map(|(name, _)| name.as_str());
    let ordered =
        names.clone().filter(|n| *n == DEFAULT_SURFACE).chain(names.filter(|n| *n != DEFAULT_SURFACE));
    Offered {
        pending: ordered.map(|name| state.adapter.send("observables", json!({ "surface": name }))).collect(),
    }
}

/// Whether the adapter implements `observables`.
pub fn can_observe(state: &State) -> bool {
    state.adapter.can("observables")
}

impl Offered {
    /// Every surface's values, in the order asked. Of several surfaces that fail, the one
    /// whose failure arrived first is the error.
    pub fn wait(self) -> Result<Vec<Observable>> {
        let mut all = Vec::new();
        for answer in in_arrival_order(self.pending)? {
            observables_of(answer, &mut all)?;
        }
        Ok(all)
    }
}

fn in_arrival_order(pending: Vec<Pending>) -> Result<Vec<Value>> {
    if pending.len() < 2 {
        return pending.into_iter().map(Pending::wait).collect();
    }
    let arrivals = &AtomicUsize::new(0);
    let answers: Vec<(usize, Result<Value>)> = std::thread::scope(|scope| {
        let waiting: Vec<_> = pending
            .into_iter()
            .map(|p| {
                scope.spawn(move || {
                    let answer = p.wait();
                    (arrivals.fetch_add(1, Ordering::SeqCst), answer)
                })
            })
            .collect();
        let lost = || (usize::MAX, Err(Error::internal("waiting for the adapter's observables failed")));
        waiting.into_iter().map(|h| h.join().unwrap_or_else(|_| lost())).collect()
    });
    let first_failure =
        answers.iter().filter(|(_, answer)| answer.is_err()).map(|(arrived, _)| *arrived).min();
    answers
        .into_iter()
        .filter(|(arrived, _)| first_failure.is_none_or(|first| *arrived == first))
        .map(|(_, answer)| answer)
        .collect()
}

fn observables_of(answer: Value, into: &mut Vec<Observable>) -> Result<()> {
    let Value::Array(items) = answer else {
        return Err(malformed());
    };
    for item in items {
        into.push(observable(item)?);
    }
    Ok(())
}

fn malformed() -> Error {
    Error::internal("observables answered something that is not a list of observables")
}

fn string(value: Option<Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

fn observable(item: Value) -> Result<Observable> {
    let Value::Object(mut o) = item else {
        return Err(malformed());
    };
    let Some(Value::Object(value)) = o.remove("value") else {
        return Err(malformed());
    };
    let value = observation(value)?;
    Ok(Observable {
        surface: string(o.remove("surface")).ok_or_else(malformed)?,
        expression: string(o.remove("expression")).ok_or_else(malformed)?,
        value,
    })
}

fn observation(mut value: serde_json::Map<String, Value>) -> Result<Observation> {
    let items = match value.remove("items") {
        None => None,
        Some(Value::Array(items)) => {
            Some(items.into_iter().map(|i| string(Some(i))).collect::<Option<Vec<String>>>())
        }
        Some(_) => Some(None),
    };
    Ok(Observation {
        is_text: value.get("kind").and_then(Value::as_str) == Some("text"),
        text: string(value.remove("text")).ok_or_else(malformed)?,
        items: items.map(|i| i.ok_or_else(malformed)).transpose()?,
        description: string(value.remove("description")).ok_or_else(malformed)?,
    })
}

/// The key of a surface value: its surface and its expression, joined by a NUL.
fn key(surface: &str, expression: &str) -> String {
    format!("{surface}\0{expression}")
}

// Each suggested expression is evaluated again, as `check` will evaluate the pasted
// directive; a suggestion whose expression does not yield the value compared is dropped.
fn verified(
    state: &State,
    suggestions: Vec<Suggestion>,
    observables: &[Observable],
) -> Result<Vec<Suggestion>> {
    let mut seen = HashSet::new();
    let unique: Vec<(String, &Suggestion)> = suggestions
        .iter()
        .map(|s| (key(&s.surface, &s.expression), s))
        .filter(|(k, _)| seen.insert(k.clone()))
        .collect();
    if unique.is_empty() {
        return Ok(suggestions);
    }
    let id = |k: usize| format!("__docattest_audit_verify__:{k}");
    let facts: Vec<Value> = unique
        .iter()
        .enumerate()
        .map(|(k, (_, s))| {
            json!({
                "id": id(k),
                "kind": "fact",
                "page": "__docattest_audit_verify__",
                "line": k + 1,
                "claim": "",
                "expression": s.expression,
                "surface": s.surface,
                "ordered": false,
            })
        })
        .collect();
    let values = state.adapter.call("evaluate", json!({ "facts": facts }))?;
    // Of two values with one expression, the later is the one compared with.
    let compared: HashMap<String, &Observation> =
        observables.iter().map(|o| (key(&o.surface, &o.expression), &o.value)).collect();
    let good: HashSet<&str> = unique
        .iter()
        .enumerate()
        .filter(|(k, (key, _))| {
            let evaluation = values.get(id(*k));
            let ok = evaluation.and_then(|e| e.get("ok")).is_some_and(truthy);
            let value = evaluation.and_then(|e| e.get("value"));
            ok && compared.get(key).is_some_and(|want| same_value(value, want))
        })
        .map(|(_, (key, _))| key.as_str())
        .collect();
    let keep: Vec<bool> =
        suggestions.iter().map(|s| good.contains(key(&s.surface, &s.expression).as_str())).collect();
    let mut keep = keep.into_iter();
    let mut suggestions = suggestions;
    suggestions.retain(|_| keep.next().unwrap_or(false));
    Ok(suggestions)
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        _ => true,
    }
}

/// Whether an evaluated value is the value compared: by their items, in any order, when both
/// are lists (a suggested fact compares a set, and a set may iterate in another order in
/// another process), else by their text.
fn same_value(evaluated: Option<&Value>, want: &Observation) -> bool {
    let items = evaluated.and_then(|v| v.get("items")).and_then(Value::as_array);
    match (items, &want.items) {
        (Some(items), Some(wanted)) => {
            let mut got: Vec<Option<&str>> = items.iter().map(Value::as_str).collect();
            let mut wanted: Vec<Option<&str>> = wanted.iter().map(|w| Some(w.as_str())).collect();
            got.sort_unstable();
            wanted.sort_unstable();
            got == wanted
        }
        _ => evaluated.and_then(|v| v.get("text")).and_then(Value::as_str) == Some(want.text.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn want(text: &str, items: Option<&[&str]>) -> Observation {
        Observation {
            text: text.into(),
            items: items.map(|i| i.iter().map(|s| (*s).to_string()).collect()),
            description: String::new(),
            is_text: false,
        }
    }

    #[test]
    fn values_are_the_same_by_items_or_by_text() {
        let list = json!({ "text": "a,b", "items": ["a", "b"], "description": "a list" });
        assert!(same_value(Some(&list), &want("different", Some(&["a", "b"]))));
        assert!(same_value(Some(&list), &want("b,a", Some(&["b", "a"]))));
        assert!(!same_value(Some(&list), &want("a,b", Some(&["a", "b", "c"]))));
        assert!(!same_value(Some(&list), &want("a,b", Some(&["a", "a"]))));
        // One side is not a list: their texts decide.
        assert!(same_value(Some(&list), &want("a,b", None)));
        assert!(!same_value(Some(&json!({ "text": "a" })), &want("a,b", Some(&["a", "b"]))));
        assert!(!same_value(None, &want("", None)));
    }

    #[test]
    fn observables_are_read_from_the_adapter() {
        let answer = json!([
            { "surface": "default", "expression": "A", "path": ["A"],
              "value": { "text": "x,y", "items": ["x", "y"], "description": "a list" } },
            { "surface": "default", "expression": "B", "path": ["B"],
              "value": { "text": "t", "description": "string \"t\"" } },
        ]);
        let mut all = Vec::new();
        observables_of(answer, &mut all).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].value.items.as_deref(), Some(&["x".to_string(), "y".to_string()][..]));
        assert_eq!(all[1].value.items, None);
        assert!(observables_of(json!({}), &mut all).is_err());
        assert!(observables_of(json!([{ "surface": "s" }]), &mut all).is_err());
    }

    #[test]
    fn the_first_failure_to_arrive_wins() {
        assert_eq!(in_arrival_order(Vec::new()), Ok(Vec::new()));
    }
}
