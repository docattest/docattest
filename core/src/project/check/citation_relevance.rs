//! Warnings when a passing citation defends a sentence that names surface exports, none of
//! which the cited test exercises.

use std::collections::{HashMap, HashSet};

use crate::adapter::Language;
use crate::api::attestation::{Attestation, Body};
use crate::api::errors::Result;
use crate::impact::mentions::code_spans_of_page;
use crate::project::check::Symbols;
use crate::project::state::State;
use crate::report::{Outcome, Status};

/// For each passing citation among `chosen` whose defended sentence has inline code spans
/// naming at least one known surface symbol (an export in `all_exports`, or any symbol
/// attested anywhere in `symbols`, such as an unexported constant of a `package main`),
/// warn when the cited test uses none of those named symbols.
pub(super) fn citation_relevance_warnings(
    state: &State,
    chosen: &[&Attestation],
    results: &[Vec<Outcome>],
    symbols: &Symbols,
    all_exports: &[String],
) -> Result<Vec<String>> {
    let known = known_symbols(all_exports, symbols);
    if known.is_empty() {
        return Ok(Vec::new());
    }
    let passing = passing_citations(chosen, results);
    if passing.is_empty() {
        return Ok(Vec::new());
    }
    let claim_spans: Vec<(&Attestation, Vec<String>)> = passing
        .into_iter()
        .map(|a| (a, candidate_spans(&a.claim, &known)))
        .filter(|(_, spans)| !spans.is_empty())
        .collect();
    if claim_spans.is_empty() {
        return Ok(Vec::new());
    }
    let named = resolve_spans(state, &claim_spans, &known)?;
    let mut warnings = Vec::new();
    for (attestation, spans) in &claim_spans {
        let Body::Test(test) = &attestation.body else { continue };
        let mentioned = mentioned_symbols(spans, &named);
        if mentioned.is_empty() {
            continue;
        }
        let used = symbols.get(&attestation.id).map_or(&[][..], Vec::as_slice);
        if !mentioned.iter().any(|name| used.iter().any(|r| r.name == *name)) {
            warnings.push(format!(
                "{}:{} citation names {}, which `{}` does not use",
                attestation.page,
                attestation.line,
                quoted_list(&mentioned),
                test.title
            ));
        }
    }
    Ok(warnings)
}

/// Union of exported surface symbols and every non-exported symbol attested anywhere in the
/// project (for example constants or entrypoint symbols of a `package main`), preserving order.
fn known_symbols(all_exports: &[String], symbols: &Symbols) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for name in all_exports {
        if seen.insert(name.clone()) {
            out.push(name.clone());
        }
    }
    let mut extra: Vec<&str> = symbols.values().flatten().map(|r| r.name.as_str()).collect();
    extra.sort_unstable();
    for name in extra {
        if seen.insert(name.to_string()) {
            out.push(name.to_string());
        }
    }
    out
}

fn passing_citations<'a>(chosen: &[&'a Attestation], results: &[Vec<Outcome>]) -> Vec<&'a Attestation> {
    chosen
        .iter()
        .zip(results)
        .filter_map(|(&a, outcomes)| {
            (matches!(a.body, Body::Test(_))
                && !outcomes.is_empty()
                && outcomes.iter().all(|o| o.status == Status::Pass))
            .then_some(a)
        })
        .collect()
}

fn candidate_spans(claim: &str, all_exports: &[String]) -> Vec<String> {
    let Some(spans) = code_spans_of_page("claim.md", claim) else { return Vec::new() };
    spans
        .values()
        .into_iter()
        .filter(|span| all_exports.iter().any(|export| span.contains(export.as_str())))
        .map(str::to_string)
        .collect()
}

fn resolve_spans(
    state: &State,
    claim_spans: &[(&Attestation, Vec<String>)],
    all_exports: &[String],
) -> Result<HashMap<String, String>> {
    let mut seen = HashSet::new();
    let mut asked: Vec<&str> = Vec::new();
    for (_, spans) in claim_spans {
        for span in spans {
            if seen.insert(span.as_str()) {
                asked.push(span.as_str());
            }
        }
    }
    let export_refs: Vec<&str> = all_exports.iter().map(String::as_str).collect();
    let answers = state.adapter.mentions(&asked, &export_refs)?;
    Ok(asked
        .into_iter()
        .zip(answers)
        .filter_map(|(span, answer)| {
            let symbol = answer.filter(|s| all_exports.iter().any(|e| e == s))?;
            Some((span.to_string(), symbol))
        })
        .collect())
}

fn mentioned_symbols(spans: &[String], named: &HashMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for span in spans {
        if let Some(symbol) = named.get(span)
            && !out.contains(symbol)
        {
            out.push(symbol.clone());
        }
    }
    out
}

fn quoted_list(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    match quoted.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_spans_filters_to_code_spans_containing_an_export() {
        let exports = vec!["PORT".to_string(), "connect".to_string()];
        assert_eq!(
            candidate_spans("`connect()` dials `PORT` on `localhost`.", &exports),
            vec!["connect()".to_string(), "PORT".to_string()]
        );
        assert!(candidate_spans("No exports named here.", &exports).is_empty());
    }

    #[test]
    fn mentioned_symbols_preserves_order_without_duplicates() {
        let named = HashMap::from([
            ("connect()".into(), "connect".into()),
            ("connect(host)".into(), "connect".into()),
            ("PORT".into(), "PORT".into()),
        ]);
        let spans = vec!["connect()".into(), "PORT".into(), "connect(host)".into(), "other".into()];
        assert_eq!(mentioned_symbols(&spans, &named), vec!["connect".to_string(), "PORT".to_string()]);
    }

    #[test]
    fn quoted_list_formats_one_two_and_more() {
        assert_eq!(quoted_list(&["a".into()]), "`a`");
        assert_eq!(quoted_list(&["a".into(), "b".into()]), "`a` and `b`");
        assert_eq!(quoted_list(&["a".into(), "b".into(), "c".into()]), "`a`, `b` and `c`");
    }

    #[test]
    fn known_symbols_includes_exports_and_non_exported_attested_symbols_without_duplicates() {
        use crate::project::check::SymbolRef;
        let symbols: Symbols = HashMap::from([(
            "id1".into(),
            vec![
                SymbolRef { name: "defaultPort".into(), surface: Some("default".into()) },
                SymbolRef { name: "PORT".into(), surface: Some("default".into()) },
            ],
        )]);
        assert_eq!(
            known_symbols(&["PORT".into()], &symbols),
            vec!["PORT".to_string(), "defaultPort".to_string()]
        );
    }
}
