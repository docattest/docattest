//! Selecting only the proofs affected by a git diff (`mutate --base <ref>`): every proof
//! `impact --base <ref>` finds (changed exports, manifests, files, commands) plus every
//! proof added or changed on Markdown pages modified since `<ref>`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::api::attestation::Attestation;
use crate::api::errors::Result;
use crate::attestations::plan::plan_parsed;
use crate::impact::{Scope, read_change, run_impact};
use crate::markdown::parse::parse_page;
use crate::project::check::Network;
use crate::project::plan::example_options;
use crate::project::state::State;

/// The attestation IDs on `selected_pages` affected by the diff from `base` to the working
/// tree. Closes the session and ends the adapter process; the caller opens a fresh session
/// for the mutation pass only when this set is non-empty.
pub fn changed_attestation_ids(
    state: State,
    network: Network,
    base: &str,
    selected_pages: &HashSet<String>,
) -> Result<HashSet<String>> {
    let change = read_change(state.files.root(), base, &Scope::Diff)?;
    let page_changes: Vec<(String, Option<String>)> =
        change.base.iter().filter(|(path, _)| selected_pages.contains(path)).cloned().collect();
    let impacted = run_impact(&state, network, base, Ok(change))?;
    let mut ids = impacted_ids(&state.attestations, &impacted.references, selected_pages);
    for (page, base_text) in page_changes {
        add_changed_page_proofs(&state, &page, base_text.as_deref(), &mut ids);
    }
    state.adapter.finish_session()?;
    Ok(ids)
}

fn impacted_ids(
    attestations: &[Attestation],
    references: &[crate::impact::ImpactReference],
    selected_pages: &HashSet<String>,
) -> HashSet<String> {
    let touched: HashSet<(&str, usize)> = references
        .iter()
        .filter(|r| r.found.is_checked() && selected_pages.contains(&r.page))
        .map(|r| (r.page.as_str(), r.line))
        .collect();
    attestations
        .iter()
        .filter(|a| touched.contains(&(a.page.as_str(), a.line)))
        .map(|a| a.id.clone())
        .collect()
}

/// Add every attestation on `page` that was added or altered relative to `base_text`.
fn add_changed_page_proofs(state: &State, page: &str, base_text: Option<&str>, ids: &mut HashSet<String>) {
    let head: Vec<&Attestation> = state.attestations.iter().filter(|a| a.page == page).collect();
    let Some(base_text) = base_text else {
        ids.extend(head.into_iter().map(|a| a.id.clone()));
        return;
    };
    let Ok(parsed) = parse_page(base_text) else {
        ids.extend(head.into_iter().map(|a| a.id.clone()));
        return;
    };
    let options = example_options(&state.config, &state.adapter);
    let base_attestations = plan_parsed(page, &parsed, &options);
    let mut remaining: HashMap<String, usize> = HashMap::new();
    for a in &base_attestations {
        *remaining.entry(signature(a)).or_default() += 1;
    }
    for a in head {
        let sig = signature(a);
        if let Some(count) = remaining.get_mut(&sig).filter(|c| **c > 0) {
            *count -= 1;
        } else {
            ids.insert(a.id.clone());
        }
    }
}

/// Content signature of an attestation ignoring its generated `id` and line number so moving
/// lines on a page does not look like a changed proof.
fn signature(attestation: &Attestation) -> String {
    let mut value = attestation.to_json();
    if let Value::Object(map) = &mut value {
        map.remove("id");
        map.remove("line");
        if let Some(Value::Array(rows)) = map.get_mut("rows") {
            for row in rows {
                if let Value::Object(row_map) = row {
                    row_map.remove("line");
                }
            }
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestations::examples::ExampleOptions;

    #[test]
    fn moved_proofs_keep_the_same_signature_while_edited_proofs_change() {
        let opts = ExampleOptions { languages: &[], ignore_tags: &[], info_words: &[] };
        let before =
            plan_parsed("docs/a.md", &parse_page("Port is `80`. <!-- fact: PORT -->\n").unwrap(), &opts);
        let moved =
            plan_parsed("docs/a.md", &parse_page("\n\nPort is `80`. <!-- fact: PORT -->\n").unwrap(), &opts);
        let edited = plan_parsed(
            "docs/a.md",
            &parse_page("Port is `80`. <!-- fact: DEFAULT_PORT -->\n").unwrap(),
            &opts,
        );
        assert_eq!(signature(&before[0]), signature(&moved[0]));
        assert_ne!(signature(&before[0]), signature(&edited[0]));
    }
}
