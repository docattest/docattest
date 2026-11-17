//! The impact as a hook hands it to an agent after each edit. It is added to the agent's
//! context on every edit, so it stays small whatever the change: it lists the places only
//! the agent can check, the prose that nothing proves, and counts the rest, which
//! `docattest check` covers.

use super::text::affected;
use super::{Impact, ImpactReference};
use crate::changes::text::quote;
use crate::counted::{agreeing, counted};
use crate::text::compare_utf16;

/// At most this many places are listed; the others are counted, with the command that lists
/// them all.
const LISTED: usize = 5;

/// The impact of an edit to `file`, `file` as the project names it, for an agent's context;
/// nothing when no page names what the edit affects, since there is nothing to re-read.
pub fn brief_impact(impact: &Impact, file: &str) -> Option<String> {
    let (unproven, checked): (Vec<Place>, Vec<Place>) =
        places(&impact.references).into_iter().partition(|place| !place.checked);
    let affects = format!("docattest: this edit affects {}", affected(impact));
    if unproven.is_empty() {
        let covered = counted(checked.len(), "place");
        let name = agreeing(checked.len(), "names", "name");
        return (!checked.is_empty())
            .then(|| format!("{affects}; docattest check covers the {covered} that {name} it."));
    }
    let mut lines = vec![format!("{affects}. Nothing checks these places; re-read them:")];
    lines.extend(unproven.iter().take(LISTED).map(Place::line));
    if unproven.len() > LISTED {
        lines.push(format!("  +{} more: docattest impact --file {file}", unproven.len() - LISTED));
    }
    if !checked.is_empty() {
        lines.push(format!("docattest check covers {}.", counted(checked.len(), "other place")));
    }
    Some(lines.join("\n"))
}

/// A line of a page that names affected symbols.
struct Place<'i> {
    page: &'i str,
    line: usize,
    symbols: Vec<&'i str>,
    claim: &'i str,
    /// Something docattest runs, compares or finds defends the claim.
    checked: bool,
}

impl Place<'_> {
    fn line(&self) -> String {
        format!("  {}:{} {} {}", self.page, self.line, self.symbols.join(", "), quote(self.claim))
    }
}

/// Each place once, in page then line order, with every symbol named there; checked when
/// any reference there is.
fn places(references: &[ImpactReference]) -> Vec<Place<'_>> {
    let mut sorted: Vec<&ImpactReference> = references.iter().collect();
    sorted.sort_by(|a, b| compare_utf16(&a.page, &b.page).then(a.line.cmp(&b.line)));
    let mut places: Vec<Place> = Vec::new();
    for r in sorted {
        match places.last_mut() {
            Some(place) if place.page == r.page && place.line == r.line => {
                if !place.symbols.contains(&r.symbol.as_str()) {
                    place.symbols.push(&r.symbol);
                }
                place.checked |= r.found.is_checked();
            }
            _ => places.push(Place {
                page: &r.page,
                line: r.line,
                symbols: vec![&r.symbol],
                claim: &r.claim,
                checked: r.found.is_checked(),
            }),
        }
    }
    places
}

#[cfg(test)]
mod tests {
    use super::super::Found;
    use super::*;
    use crate::api::attestation::Kind;

    fn reference(symbol: &str, page: &str, line: usize, found: Found, claim: &str) -> ImpactReference {
        ImpactReference { symbol: symbol.into(), page: page.into(), line, found, claim: claim.into() }
    }

    fn impact(references: Vec<ImpactReference>) -> Impact {
        Impact {
            base: "HEAD".into(),
            symbols: vec!["Client".into(), "parse".into()],
            manifests: vec![],
            commands: vec![],
            references,
        }
    }

    #[test]
    fn lists_each_unproven_place_once_and_at_most_five_and_counts_what_check_covers() {
        let mut references: Vec<ImpactReference> = (1..=7)
            .map(|line| reference("parse", "docs/guide.md", line, Found::Mention, "Read it with `parse`."))
            .collect();
        references.push(reference("Client", "docs/guide.md", 2, Found::Mention, "Read it with `parse`."));
        references.push(reference("parse", "docs/parse.md", 5, Found::Proven(Kind::Example), "A number."));
        let text = brief_impact(&impact(references), "src/index.ts").expect("a brief");
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            [
                "docattest: this edit affects 2 exported symbols. Nothing checks these places; re-read them:",
                "  docs/guide.md:1 parse \"Read it with `parse`.\"",
                "  docs/guide.md:2 parse, Client \"Read it with `parse`.\"",
                "  docs/guide.md:3 parse \"Read it with `parse`.\"",
                "  docs/guide.md:4 parse \"Read it with `parse`.\"",
                "  docs/guide.md:5 parse \"Read it with `parse`.\"",
                "  +2 more: docattest impact --file src/index.ts",
                "docattest check covers 1 other place.",
            ]
        );
    }

    #[test]
    fn a_place_with_a_proof_is_covered_and_no_place_is_nothing_to_say() {
        let proven = vec![
            reference("parse", "docs/a.md", 3, Found::Mention, "Use `parse`."),
            reference("Client", "docs/a.md", 3, Found::Proven(Kind::Fact), "Use `parse`."),
        ];
        assert_eq!(
            brief_impact(&impact(proven), "src/index.ts").as_deref(),
            Some(
                "docattest: this edit affects 2 exported symbols; docattest check covers the 1 place that names it."
            )
        );
        assert_eq!(brief_impact(&impact(vec![]), "src/index.ts"), None);
    }
}
