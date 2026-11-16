//! What happened to an attestation that survived: did its proof get weaker?

use std::collections::{HashMap, HashSet};

use super::change::{Change, ChangeType};
use super::signature::target_of;
use super::text::squash;
use crate::adapter::{ExpressionShape, Language};
use crate::api::attestation::{Attestation, Body, Example, Fact, Mode};
use crate::api::errors::Result;

/// The expressions that became another only because the code they named was renamed: the
/// same shape with other names, and every name dropped is gone from the code. The page did
/// not get weaker. Each is the squashed expression before and after.
pub type Renamed = HashSet<(String, String)>;

/// A downgrade or a weakening between a base attestation and the one it became.
pub fn judge_pair(base: &Attestation, head: &Attestation, renamed: &Renamed) -> Option<Change> {
    if base.problem().is_some() || head.problem().is_some() {
        return None;
    }
    match (&base.body, &head.body) {
        (Body::Example(before), Body::Example(after)) => downgrade(before, after, head),
        (Body::Fact(before), Body::Fact(after)) => weakened(base, before, head, after, renamed),
        _ => None,
    }
}

fn warning(kind: ChangeType, at: &Attestation, message: String) -> Option<Change> {
    Some(Change::warning(kind, &at.page, at.line, message))
}

fn downgrade(base: &Example, head: &Example, at: &Attestation) -> Option<Change> {
    if base.mode != Mode::Check {
        return None;
    }
    if head.mode == Mode::Compile {
        return warning(ChangeType::Downgrade, at, "the example became `compile`: it no longer runs".into());
    }
    if !base.weak && head.weak {
        let message = "the example lost every assertion: it runs, proving nothing";
        return warning(ChangeType::Downgrade, at, message.into());
    }
    None
}

fn weakened(
    base_at: &Attestation,
    base: &Fact,
    head_at: &Attestation,
    head: &Fact,
    renamed: &Renamed,
) -> Option<Change> {
    let target = target_of(base_at);
    if target.is_empty() || target != target_of(head_at) {
        return None;
    }
    let before = squash(&base.expression);
    let after = squash(&head.expression);
    if base.ordered && !head.ordered && before == after {
        let message = "the fact no longer checks order, and the page did not change";
        return warning(ChangeType::Weakened, head_at, message.into());
    }
    let same_surface = base.surface == head.surface;
    if before == after && same_surface {
        return None;
    }
    if same_surface && renamed.contains(&(before.clone(), after.clone())) {
        return None;
    }
    warning(ChangeType::Weakened, head_at, expression_changed(base, head, &before, &after))
}

fn expression_changed(base: &Fact, head: &Fact, before: &str, after: &str) -> String {
    let (from, to) = if base.surface == head.surface {
        (format!("`{before}`"), format!("`{after}`"))
    } else {
        (format!("`{before}` @{}", base.surface), format!("`{after}` @{}", head.surface))
    };
    format!("the fact's expression changed from {from} to {to}; the literal it defends did not")
}

/// The expressions [`weakened`] would ask about: a fact that kept its target and its surface
/// and changed its expression. Squashed, before and after.
fn rewritten(pairs: &[(&Attestation, &Attestation)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (base, head) in pairs {
        if base.problem().is_some() || head.problem().is_some() {
            continue;
        }
        let (Body::Fact(before), Body::Fact(after)) = (&base.body, &head.body) else { continue };
        let target = target_of(base);
        if before.surface != after.surface || target.is_empty() || target != target_of(head) {
            continue;
        }
        let (before, after) = (squash(&before.expression), squash(&after.expression));
        if before != after {
            out.push((before, after));
        }
    }
    out
}

/// Find the renames among the pairs. What a name is and what an expression's shape is are the
/// language's to say, and which names the code still holds is its session's, so the pairs are
/// asked about together: one request for the shapes, and one for the names if any expression
/// kept its shape. `sources` lists the head's files that count as its code; it is called
/// only if that second request is made.
pub fn renames(
    pairs: &[(&Attestation, &Attestation)],
    language: &dyn Language,
    sources: &dyn Fn() -> Result<Vec<String>>,
) -> Result<Renamed> {
    let changed = rewritten(pairs);
    if changed.is_empty() {
        return Ok(Renamed::new());
    }
    let texts = distinct(changed.iter().flat_map(|(before, after)| [before.as_str(), after.as_str()]));
    let answers = language.expressions(&texts)?;
    let shapes: HashMap<&str, &ExpressionShape> = texts.iter().copied().zip(&answers).collect();
    let candidates = same_shapes(&changed, &shapes);
    let names = distinct(candidates.iter().flat_map(|(_, dropped)| dropped.iter().copied()));
    let still: HashSet<String> = if names.is_empty() {
        HashSet::new()
    } else {
        language.in_code(&names, &sources()?)?.into_iter().collect()
    };
    Ok(candidates
        .into_iter()
        .filter(|(_, dropped)| dropped.iter().all(|name| !still.contains(*name)))
        .map(|(pair, _)| pair.clone())
        .collect())
}

/// Each text once, in the order first seen.
fn distinct<'t>(texts: impl Iterator<Item = &'t str>) -> Vec<&'t str> {
    let mut out: Vec<&str> = Vec::new();
    for text in texts {
        if !out.contains(&text) {
            out.push(text);
        }
    }
    out
}

/// The pairs whose expressions kept their shape and dropped names, with the names dropped.
fn same_shapes<'p>(
    changed: &'p [(String, String)],
    shapes: &HashMap<&str, &'p ExpressionShape>,
) -> Vec<(&'p (String, String), Vec<&'p str>)> {
    let mut candidates = Vec::new();
    for pair in changed {
        let (Some(was), Some(is)) = (shapes.get(pair.0.as_str()), shapes.get(pair.1.as_str())) else {
            continue;
        };
        if was.shape != is.shape {
            continue;
        }
        let dropped: Vec<&str> =
            was.names.iter().filter(|name| !is.names.contains(name)).map(String::as_str).collect();
        if !dropped.is_empty() {
            candidates.push((pair, dropped));
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::fake::Typescript;
    use crate::api::attestation::FactTarget;

    fn fact(expression: &str, surface: &str) -> Attestation {
        Attestation {
            id: "p.md:1:fact:0".into(),
            page: "p.md".into(),
            line: 1,
            claim: "c".into(),
            error: None,
            body: Body::Fact(Fact {
                expression: expression.into(),
                surface: surface.into(),
                ordered: false,
                column: None,
                target: Ok(FactTarget::Scalar("3".into())),
            }),
        }
    }

    fn renamed(code: &str, before: &str, after: &str) -> bool {
        let files = vec![("src/a.ts".to_string(), code.to_string())];
        let language = Typescript::open(&files, &[]);
        let (base, head) = (fact(before, "default"), fact(after, "default"));
        let found = renames(&[(&base, &head)], &language, &|| Ok(vec!["src/a.ts".into()])).unwrap();
        found.contains(&(squash(before), squash(after)))
    }

    #[test]
    fn a_rename_keeps_the_shape_and_drops_names_the_code_no_longer_has() {
        let gone = "export const MAX_TRIES = 3; const config = { MAX: 1 };";
        assert!(renamed(gone, "LIMIT", "MAX_TRIES"));
        assert!(renamed(gone, "config.LIMIT + 1", "config.MAX  +  1"));
        assert!(!renamed(gone, "LIMIT", "Math.min(LIMIT, 3)"));
        assert!(!renamed(gone, "LIMIT + 1", "MAX + 2"));
        // The old name is still in the code: the expression now names something else.
        assert!(!renamed("export const LIMIT = 3, OTHER = 3;", "LIMIT", "OTHER"));
        // Nothing was dropped, or nothing changed.
        assert!(!renamed(gone, "a + b", "b + a"));
        assert!(!renamed(gone, "a + a", "a + a"));
    }

    #[test]
    fn only_a_fact_that_kept_its_target_and_surface_is_asked_about() {
        let (a, b) = (fact("LIMIT", "default"), fact("MAX", "default"));
        assert_eq!(rewritten(&[(&a, &b)]), [("LIMIT".to_string(), "MAX".to_string())]);
        assert!(rewritten(&[(&a, &a)]).is_empty());
        assert!(rewritten(&[(&a, &fact("MAX", "other"))]).is_empty());
        let mut failed = fact("MAX", "default");
        failed.error = Some("x".into());
        assert!(rewritten(&[(&a, &failed)]).is_empty());
    }
}
