//! From candidates and the inventory to suggestions: the best surface value for each list, table
//! and block, or nothing when the best is not clear. Sentences and single values are `inline`'s.

use std::collections::HashSet;

use super::candidates::{BlockCandidate, PageCandidates, SetCandidate};
use super::inline::inline_suggestions;
use super::inventory::Inventory;
use super::matching::{MIN_SHARED, Verdict, compare_block, compare_sets};
use super::{Observable, Suggestion, Target};
use crate::config::DEFAULT_SURFACE;
use crate::text::is_js_space;

/// A surface value, and how a candidate compares with it.
pub(super) struct Ranked<'a> {
    pub verdict: Verdict,
    pub observable: &'a Observable,
}

/// Suggestions for one page, in line order, never two that cannot be told apart. Nothing is
/// suggested inline inside a list or table that has a suggestion of its own.
pub fn suggest_for_page(page: &str, found: &PageCandidates, inventory: &Inventory) -> Vec<Suggestion> {
    let mut covered: Vec<(usize, usize)> = Vec::new();
    let mut all: Vec<Suggestion> = Vec::new();
    for group in &found.sets {
        if let Some(suggestion) = best_column(page, group, inventory) {
            covered.extend(group.first().map(|c| (c.line, c.end)));
            all.push(suggestion);
        }
    }
    all.extend(found.blocks.iter().filter_map(|b| block_suggestion(page, b, inventory)));
    let mut taken = found.attested.clone();
    taken.extend(all.iter().map(|s| (s.surface.clone(), s.expression.clone())));
    let outside = |line: usize| !covered.iter().any(|(start, end)| (*start..=*end).contains(&line));
    let sentences = found.sentences.iter().filter(|s| outside(s.line));
    all.extend(inline_suggestions(page, sentences, inventory, &mut taken));
    let mut all = distinct(all);
    all.sort_by_key(|s| s.line);
    all
}

// One suggestion per list or table: a table's columns compete, and the strongest wins.
fn best_column(page: &str, group: &[SetCandidate], inventory: &Inventory) -> Option<Suggestion> {
    let mut best: Option<(Ranked, &SetCandidate)> = None;
    for candidate in group {
        if candidate.column.is_some() && column_name(candidate, group).is_none() {
            continue;
        }
        // Only lists sharing enough items could pass `compare_sets`; the index finds them.
        let lists = inventory.sharing(&candidate.items, MIN_SHARED);
        let verdicts = lists.into_iter().filter_map(|o| {
            let items = o.value.items.as_deref().unwrap_or(&[]);
            compare_sets(&candidate.items, items).map(|verdict| Ranked { verdict, observable: o })
        });
        let Some(ranked) = pick_with_headings(verdicts, &candidate.headings) else { continue };
        if best.as_ref().is_none_or(|(b, _)| stronger(&ranked.verdict, &b.verdict)) {
            best = Some((ranked, candidate));
        }
    }
    let (ranked, candidate) = best?;
    let column = column_name(candidate, group);
    Some(build(page, candidate.line, candidate.target.clone(), ranked, column.as_deref()))
}

fn block_suggestion(page: &str, block: &BlockCandidate, inventory: &Inventory) -> Option<Suggestion> {
    let verdicts = inventory.strings.iter().filter_map(|o| {
        compare_block(&block.text, &o.value).map(|verdict| Ranked { verdict, observable: o })
    });
    pick(verdicts).map(|ranked| build(page, block.line, Target::Block, ranked, None))
}

fn stronger(a: &Verdict, b: &Verdict) -> bool {
    a.score > b.score || (a.score == b.score && a.weight > b.weight)
}

/// The strongest verdict, or None when another of equal strength disagrees about what is
/// missing or unexpected: two values fit equally well, and guessing would mislead.
pub(super) fn pick<'a>(ranked: impl Iterator<Item = Ranked<'a>>) -> Option<Ranked<'a>> {
    pick_with_headings(ranked, &[])
}

/// Like [`pick`], breaking equal-verdict ties with identical differences by preferring the
/// expression whose identifier tokens overlap most with the enclosing Markdown headings.
fn pick_with_headings<'a>(
    ranked: impl Iterator<Item = Ranked<'a>>,
    headings: &[String],
) -> Option<Ranked<'a>> {
    let context = heading_tokens(headings);
    let mut best: Option<(Ranked<'a>, usize)> = None;
    let mut tied = false;
    for r in ranked {
        let overlap = if context.is_empty() { 0 } else { token_overlap(&r.observable.expression, &context) };
        match &best {
            Some((b, best_overlap)) if !stronger(&r.verdict, &b.verdict) => {
                if stronger(&b.verdict, &r.verdict) {
                    continue;
                }
                if !same_difference(&b.verdict, &r.verdict) {
                    tied = true;
                } else if overlap > *best_overlap {
                    best = Some((r, overlap));
                }
            }
            _ => {
                best = Some((r, overlap));
                tied = false;
            }
        }
    }
    if tied { None } else { best.map(|(r, _)| r) }
}

fn heading_tokens(headings: &[String]) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for heading in headings {
        tokens.extend(identifier_tokens(heading));
        for word in heading.split_whitespace() {
            let joined: String =
                word.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
            if joined.chars().count() >= 2 {
                tokens.insert(joined);
            }
        }
    }
    tokens
}

fn token_overlap(expression: &str, context: &HashSet<String>) -> usize {
    identifier_tokens(expression).into_iter().filter(|tok| context.contains(tok)).count()
}

/// Split an identifier or heading into lowercase alphanumeric words across punctuation and
/// camelCase boundaries.
fn identifier_tokens(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut current = String::new();
    let mut prev: Option<char> = None;
    for ch in text.chars() {
        if !ch.is_alphanumeric() {
            push_token(&mut out, &mut current);
            prev = None;
            continue;
        }
        if prev.is_some_and(|p| p.is_lowercase() && ch.is_uppercase()) {
            push_token(&mut out, &mut current);
        }
        current.extend(ch.to_lowercase());
        prev = Some(ch);
    }
    push_token(&mut out, &mut current);
    out
}

fn push_token(out: &mut HashSet<String>, current: &mut String) {
    if current.chars().count() >= 2 {
        out.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn same_difference(a: &Verdict, b: &Verdict) -> bool {
    a.missing == b.missing && a.unexpected == b.unexpected
}

fn distinct(suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
    let mut seen: HashSet<(usize, Target, String, String)> = HashSet::new();
    suggestions
        .into_iter()
        .filter(|s| seen.insert((s.line, s.target.clone(), s.surface.clone(), s.expression.clone())))
        .collect()
}

// `column=` finds the first header cell that equals its name: a repeated or unquotable
// header cannot be named, so its column is never suggested.
fn column_name(candidate: &SetCandidate, group: &[SetCandidate]) -> Option<String> {
    let name = candidate.column.as_deref()?;
    if name.is_empty() || name.contains("-->") {
        return None;
    }
    let first = group.iter().find(|c| c.column.as_deref() == Some(name))?;
    if !std::ptr::eq(first, candidate) {
        return None;
    }
    // `/^[^\s:"']+$/`
    if !name.chars().any(|c| is_js_space(c) || matches!(c, ':' | '"' | '\'')) {
        return Some(name.to_string());
    }
    if !name.contains('"') {
        return Some(format!("\"{name}\""));
    }
    if name.contains('\'') { None } else { Some(format!("'{name}'")) }
}

pub(super) fn build(
    page: &str,
    line: usize,
    target: Target,
    ranked: Ranked,
    column: Option<&str>,
) -> Suggestion {
    let Ranked { verdict, observable } = ranked;
    let Observable { surface, expression, .. } = observable;
    let at = if surface == DEFAULT_SURFACE { String::new() } else { format!(" @{surface}") };
    let column = column.map(|c| format!(" column={c}")).unwrap_or_default();
    Suggestion {
        page: page.to_string(),
        line,
        target,
        expression: expression.clone(),
        status: verdict.status,
        missing: verdict.missing,
        unexpected: verdict.unexpected,
        surface: surface.clone(),
        directive: crate::markdown::mdx::format_directive(page, &format!("fact{at}{column}: {expression}")),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::candidates::candidates_of;
    use super::super::matching::tests::{observe_list, observe_text};
    use super::*;

    pub(crate) fn value(expression: &str, items: &[&str]) -> Observable {
        Observable { surface: "default".into(), expression: expression.into(), value: observe_list(items) }
    }

    pub(crate) fn suggest(text: &str, observables: &[Observable]) -> Vec<Suggestion> {
        suggest_for_page("docs/p.md", &candidates_of(text).unwrap(), &Inventory::new(observables))
    }

    fn brief(text: &str, observables: &[Observable]) -> Vec<(usize, &'static str, &'static str, String)> {
        suggest(text, observables)
            .into_iter()
            .map(|s| (s.line, s.target.as_str(), s.status.as_str(), s.directive))
            .collect()
    }

    fn one(
        line: usize,
        target: &'static str,
        status: &'static str,
        directive: &str,
    ) -> Vec<(usize, &'static str, &'static str, String)> {
        vec![(line, target, status, directive.to_string())]
    }

    #[test]
    fn picks_the_best_value_for_a_list_preferring_the_earlier_of_equal_ones() {
        let page = "- `RPC`\n- `SESSION`\n";
        let keys = value("Object.keys(ErrorCodes)", &["RPC", "SESSION"]);
        let values = value("Object.values(ErrorCodes)", &["RPC", "SESSION"]);
        assert_eq!(
            brief(page, &[keys, values]),
            one(1, "list", "match", "<!-- fact: Object.keys(ErrorCodes) -->")
        );
        let closer = value("BOTH", &["RPC", "SESSION"]);
        let found = brief(page, &[value("MORE", &["RPC", "SESSION", "X1"]), closer]);
        assert_eq!(found[0].3, "<!-- fact: BOTH -->");
    }

    #[test]
    fn stays_silent_when_two_values_fit_equally_and_disagree() {
        let page = "- `aa`\n- `bb`\n";
        let found = brief(page, &[value("X", &["aa", "bb", "cc"]), value("Y", &["aa", "bb", "dd"])]);
        assert_eq!(found, vec![]);
    }

    #[test]
    fn suggests_one_column_per_table_the_strongest_naming_it_as_column_can() {
        let page = "| Method | Request path |\n|---|---|\n| GET | `/jobs` |\n| POST | `/projects` |\n";
        let found = brief(
            page,
            &[
                value("ROUTES.map((r) => r.method)", &["GET", "POST", "PUT"]),
                value("ROUTES.map((r) => r.path)", &["/jobs", "/projects", "/x"]),
            ],
        );
        assert_eq!(
            found,
            one(1, "column", "drift", "<!-- fact column=\"Request path\": ROUTES.map((r) => r.path) -->")
        );
    }

    #[test]
    fn never_suggests_a_column_whose_header_cannot_be_named() {
        let page = "| A | A |\n|---|---|\n| x | `p1` |\n| y | `p2` |\n";
        assert_eq!(brief(page, &[value("P", &["p1", "p2"])]), vec![]);
    }

    #[test]
    fn quotes_a_header_as_column_can_read_it() {
        let table = |header: &str| format!("| {header} |\n|---|\n| `p1` |\n| `p2` |\n");
        let directive = |header: &str| brief(&table(header), &[value("P", &["p1", "p2"])]).pop().map(|s| s.3);
        assert_eq!(directive("a:b").as_deref(), Some("<!-- fact column=\"a:b\": P -->"));
        assert_eq!(directive("say \"x\"").as_deref(), Some("<!-- fact column='say \"x\"': P -->"));
        assert_eq!(directive("it's \"x\""), None);
        assert_eq!(directive("a\u{A0}b").as_deref(), Some("<!-- fact column=\"a\u{A0}b\": P -->"));
        assert_eq!(directive("Größe").as_deref(), Some("<!-- fact column=Größe: P -->"));
    }

    #[test]
    fn names_a_surface_other_than_the_default() {
        let mut p = value("P", &["p1", "p2"]);
        p.surface = "server".into();
        assert_eq!(brief("- `p1`\n- `p2`\n", &[p]), one(1, "list", "match", "<!-- fact @server: P -->"));
    }

    #[test]
    fn prefers_the_deeper_set_that_fits_better_and_is_silent_when_unrelated_sets_tie() {
        let table = "| Name | Note |\n|---|---|\n| `job` | a |\n| `project` | b |\n| `owner` | c |\n";
        let events = value("Object.values(EventType)", &["job", "project", "run"]);
        let fields = value("Object.keys(Jobs.CLOSE.fields)", &["job", "project", "owner", "archive"]);
        assert_eq!(
            brief(table, &[events.clone(), fields.clone()]),
            one(1, "column", "drift", "<!-- fact column=Name: Object.keys(Jobs.CLOSE.fields) -->")
        );
        let other = value("Object.keys(Jobs.LIST.fields)", &["job", "project", "owner", "run"]);
        assert_eq!(brief(table, &[events, fields, other]), vec![]);
    }

    #[test]
    fn never_makes_two_suggestions_that_cannot_be_told_apart() {
        let page = "```yaml\nname: demo\nregion: eu\nport: 8080\n```\n";
        let text = "name: demo\nregion: eu\nport: 8080\n";
        let template = || Observable {
            surface: "default".into(),
            expression: "TEMPLATE".into(),
            value: observe_text(text),
        };
        assert_eq!(
            brief(page, &[template(), template()]),
            one(1, "block", "match", "<!-- fact: TEMPLATE -->")
        );
    }

    #[test]
    fn breaks_equal_set_ties_using_enclosing_headings() {
        let page = [
            "# Vault",
            "## Close",
            "### Accounts",
            "| Name | Role |",
            "|---|---|",
            "| `user` | owner |",
            "| `vault` | pool |",
            "## Deposit",
            "### Accounts",
            "| Name | Role |",
            "|---|---|",
            "| `user` | owner |",
            "| `vault` | pool |",
            "## Withdraw",
            "### Accounts",
            "| Name | Role |",
            "|---|---|",
            "| `user` | owner |",
            "| `vault` | pool |",
        ]
        .join("\n")
            + "\n";
        let observables = [
            value("Vault.CLOSE_ACCOUNTS", &["user", "vault"]),
            value("Vault.DEPOSIT_ACCOUNTS", &["user", "vault"]),
            value("Vault.WITHDRAW_ACCOUNTS", &["user", "vault"]),
        ];
        assert_eq!(
            brief(&page, &observables),
            vec![
                (4, "column", "match", "<!-- fact column=Name: Vault.CLOSE_ACCOUNTS -->".into()),
                (10, "column", "match", "<!-- fact column=Name: Vault.DEPOSIT_ACCOUNTS -->".into()),
                (16, "column", "match", "<!-- fact column=Name: Vault.WITHDRAW_ACCOUNTS -->".into()),
            ]
        );
    }
}
