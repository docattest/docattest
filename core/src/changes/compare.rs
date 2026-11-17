//! What the head did to the attestations of the base.

use std::collections::HashSet;

use super::change::Change;
use super::frozen::frozen_pages;
use super::judge::{judge_pair, renames};
use super::matching::{Matching, match_attestations};
use super::orphans::lost_proof;
use super::project::Project;
use super::side::{Side, prose_of};
use crate::api::attestation::Attestation;
use crate::api::errors::Result;
use crate::config::matcher;
use crate::text::compare_utf16;

/// What the head did to the attestations of the base, parse-only: frozen pages, orphaned
/// claims, downgraded examples and weakened facts. Ordered by page, line and type. A base
/// without a config (`None`) had nothing to protect. The head's language is asked what the
/// core cannot read itself: the prose of doc comments, and which changed expressions are
/// renames (which needs its session open).
pub fn changes_between(base: Option<&Project>, head: &Project) -> Result<Vec<Change>> {
    let Some(base) = base else { return Ok(Vec::new()) };
    let (base, now) = sides(base, head)?;
    let frozen = frozen_pages(&base, &now)?;
    let matching = match_attestations(&base, &now)?;
    let pairs: Vec<(&Attestation, &Attestation)> = matching
        .pairs
        .iter()
        .map(|&(b, h)| (&base.project.attestations[b], &now.project.attestations[h]))
        .collect();
    let renamed = renames(&pairs, head.language, &|| head_sources(head))?;
    let mut changes = frozen.changes;
    for (before, after) in pairs {
        changes.extend(judge_pair(before, after, &renamed));
    }
    changes.extend(vanished(&base, &now, &matching, &frozen.pages)?);
    Ok(unique(changes))
}

/// Both sides, each with the prose of its doc comments.
fn sides<'a>(base: &Project<'a>, head: &Project<'a>) -> Result<(Side<'a>, Side<'a>)> {
    let [base_prose, head_prose] = prose_of(base, head)?;
    Ok((Side::new(*base, base_prose)?, Side::new(*head, head_prose)?))
}

/// The head's code is its sources; the session adds the files its surfaces are read from.
fn head_sources(head: &Project) -> Result<Vec<String>> {
    let is_source = matcher(&head.config.sources)?;
    Ok(head.files.list().into_iter().filter(|f| is_source.is_match(f)).collect())
}

/// What became of the base attestations with no counterpart, on pages not frozen.
fn vanished(base: &Side, now: &Side, matching: &Matching, frozen: &HashSet<String>) -> Result<Vec<Change>> {
    let became: HashSet<usize> = matching.pairs.iter().map(|&(_, h)| h).collect();
    let fresh: HashSet<&str> = (now.project.attestations.iter().enumerate())
        .filter(|(k, _)| !became.contains(k))
        .map(|(_, h)| h.id.as_str())
        .collect();
    let mut changes = Vec::new();
    for &k in &matching.unmatched {
        let a = &base.project.attestations[k];
        if frozen.contains(&a.page) {
            continue;
        }
        changes.extend(lost_proof(a, base, now, &matching.counterparts(now, &a.page), &fresh)?);
    }
    Ok(changes)
}

// Two proofs of one sentence that both vanished orphan it once.
fn unique(mut changes: Vec<Change>) -> Vec<Change> {
    changes.sort_by(|a, b| {
        compare_utf16(&a.page, &b.page)
            .then(a.line.cmp(&b.line))
            .then_with(|| compare_utf16(a.kind.as_str(), b.kind.as_str()))
    });
    let mut seen = HashSet::new();
    changes.retain(|c| seen.insert((c.kind.as_str(), c.page.clone(), c.line)));
    changes
}
