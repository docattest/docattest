//! Suggestions for values copied into prose: the code spans of a sentence that match a surface
//! list as a set, and a single distinctive value that equals exactly one surface value.
//!
//! A page mentions a value more often than it states it, so each surface value is suggested
//! once per page, where it first appears, and not at all on a page where a fact already reads
//! it: one proof on the page holds the value.

use std::collections::HashSet;

use super::candidates::is_comparable;
use super::inventory::Inventory;
use super::matching::{MIN_SHARED, Verdict, compare_sets, is_distinctive};
use super::prose::{Literal, SentenceCandidate};
use super::suggest::{Ranked, build, pick};
use super::{Status, Suggestion, Target};

/// The surface and expression of each value already proved or suggested on the page.
pub type Taken = HashSet<(String, String)>;

/// Suggestions for a page's sentences, in their order; each one suggested is added to `taken`.
pub fn inline_suggestions<'a>(
    page: &str,
    sentences: impl Iterator<Item = &'a SentenceCandidate>,
    inventory: &Inventory,
    taken: &mut Taken,
) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for sentence in sentences {
        if let Some(ranked) = enumeration(sentence, inventory) {
            // A sentence that copies a list is that list: its values are not read one by one.
            if claim(&ranked, taken) {
                out.push(build(page, sentence.line, Target::Inline, ranked, None));
            }
            continue;
        }
        for literal in &sentence.values {
            let Some(ranked) = single_value(literal, inventory) else { continue };
            if claim(&ranked, taken) {
                let target = Target::Scalar { after: literal.written.clone() };
                out.push(build(page, literal.line, target, ranked, None));
            }
        }
    }
    out
}

/// Whether the value `ranked` reads is not taken yet; takes it.
fn claim(ranked: &Ranked, taken: &mut Taken) -> bool {
    taken.insert((ranked.observable.surface.clone(), ranked.observable.expression.clone()))
}

/// The surface list a sentence's code spans copy, by the rules a list is held to.
fn enumeration<'a>(sentence: &SentenceCandidate, inventory: &Inventory<'a>) -> Option<Ranked<'a>> {
    if !is_comparable(&sentence.spans) {
        return None;
    }
    let lists = inventory.sharing(&sentence.spans, MIN_SHARED);
    pick(lists.into_iter().filter_map(|o| {
        let items = o.value.items.as_deref().unwrap_or(&[]);
        compare_sets(&sentence.spans, items).map(|verdict| Ranked { verdict, observable: o })
    }))
}

/// The one surface value a distinctive literal equals.
fn single_value<'a>(literal: &Literal, inventory: &Inventory<'a>) -> Option<Ranked<'a>> {
    if !is_distinctive(&literal.value) {
        return None;
    }
    let observable = inventory.only_value(&literal.value)?;
    let verdict =
        Verdict { status: Status::Match, missing: Vec::new(), unexpected: Vec::new(), score: 1.0, weight: 0 };
    Some(Ranked { verdict, observable })
}

#[cfg(test)]
mod tests {
    use super::super::candidates::candidates_of;
    use super::super::matching::tests::observe_text;
    use super::super::suggest::suggest_for_page;
    use super::super::suggest::tests::value;
    use super::*;
    use crate::audit::Observable;

    fn scalar(expression: &str, text: &str) -> Observable {
        Observable { surface: "default".into(), expression: expression.into(), value: observe_text(text) }
    }

    /// Each suggestion's line, target, directive and where it goes.
    fn brief(text: &str, observables: &[Observable]) -> Vec<(usize, String, String)> {
        let found = candidates_of(text).unwrap();
        suggest_for_page("docs/p.md", &found, &Inventory::new(observables))
            .into_iter()
            .map(|s| {
                let place = match &s.target {
                    Target::Scalar { after } => format!("after {after}"),
                    other => other.as_str().to_string(),
                };
                (s.line, place, s.directive)
            })
            .collect()
    }

    fn one(line: usize, place: &str, directive: &str) -> (usize, String, String) {
        (line, place.to_string(), directive.to_string())
    }

    #[test]
    fn a_sentence_that_names_a_list_is_suggested_at_its_end() {
        let page = "Blocks tagged `python`,\n`py` or `pycon` are ours. Then `other` and `more`.\n";
        let languages = value("LANGUAGES", &["python", "py", "pycon"]);
        assert_eq!(brief(page, &[languages]), [one(2, "inline", "<!-- fact: LANGUAGES -->")]);
    }

    #[test]
    fn a_distinctive_value_equal_to_one_surface_value_is_suggested_right_after_it() {
        let page = "Set `DOCATTEST_CORE` or port 8080. Not `queue`, not 90, not `EUR`.\n";
        let surface = [
            scalar("CORE", "DOCATTEST_CORE"),
            scalar("PORT", "8080"),
            scalar("EventType.QUEUE", "queue"),
            scalar("TIMEOUT", "90"),
            scalar("CURRENCY", "EUR"),
        ];
        assert_eq!(
            brief(page, &surface),
            [
                one(1, "after `DOCATTEST_CORE`", "<!-- fact: CORE -->"),
                one(1, "after 8080", "<!-- fact: PORT -->")
            ]
        );
        let shared = [scalar("A", "DOCATTEST_CORE"), scalar("B", "DOCATTEST_CORE")];
        assert_eq!(brief(page, &shared), []);
    }

    #[test]
    fn a_value_after_other_code_spans_is_suggested_right_after_it() {
        let page = "Use `a1` or `DOCATTEST_CORE` here.\n";
        let found = brief(page, &[scalar("CORE", "DOCATTEST_CORE")]);
        assert_eq!(found, [one(1, "after `DOCATTEST_CORE`", "<!-- fact: CORE -->")]);
    }

    #[test]
    fn a_value_is_suggested_once_per_page_and_never_where_a_fact_reads_it() {
        let surface = [scalar("CORE", "DOCATTEST_CORE")];
        let twice = "Set `DOCATTEST_CORE`.\n\nAgain, `DOCATTEST_CORE`.\n";
        assert_eq!(brief(twice, &surface), [one(1, "after `DOCATTEST_CORE`", "<!-- fact: CORE -->")]);
        let proved = "Set `DOCATTEST_CORE`.\n\nIt is `DOCATTEST_CORE`. <!-- fact: CORE -->\n";
        assert_eq!(brief(proved, &surface), []);
    }

    #[test]
    fn nothing_inline_inside_a_list_that_has_a_suggestion() {
        let page = "- `python` and `DOCATTEST_CORE`.\n- `py`\n- `pycon`\n";
        let surface = [value("LANGUAGES", &["python", "py", "pycon"]), scalar("CORE", "DOCATTEST_CORE")];
        assert_eq!(brief(page, &surface), [one(1, "list", "<!-- fact: LANGUAGES -->")]);
    }

    #[test]
    fn suggests_distinctive_scalars_even_in_test_cited_sentences_and_skips_single_segment_paths() {
        let page = "- Blocks `/etc/ld.so.preload` injection. <!-- test: guard_test.go > \"TestPreload\" -->\n\
                    - Reads `/proc` entries.\n";
        let surface = [scalar("preloadFile", "/etc/ld.so.preload"), scalar("procRoot", "/proc")];
        assert_eq!(
            brief(page, &surface),
            [one(1, "after `/etc/ld.so.preload`", "<!-- fact: preloadFile -->")]
        );
    }
}
