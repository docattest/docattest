//! `explain <page>[:<line>]`: what each directive on the line, or on the whole page, binds and
//! why, the sentence it defends, and what checking it gives, as `check` gives it.

use std::path::Path;

use super::args::Options;
use super::check::page_argument;
use super::{Io, finish, load, network, pretty};
use crate::api::attestation::Attestation;
use crate::api::errors::{Error, Result};
use crate::attestations::alias::Alias;
use crate::attestations::plan::{Explained, Why, plan_explained};
use crate::config::matcher;
use crate::explain::{Explanation, format, to_json};
use crate::files::project_files::ProjectFiles;
use crate::markdown::parse::parse_page;
use crate::project::check::results_of;
use crate::project::plan::example_options;
use crate::project::state::{State, select_pages};

const USAGE: &str = "usage: docattest explain <page>[:<line>]";

/// `explain`: exit 0 whatever the results say; a page or line with no directive is an error.
pub(super) fn explain(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let [target] = args else { return Err(Error::user(USAGE)) };
    let (page, line) = parse_target(target)?;
    let state = load(root)?;
    let page = one_page(&state, &page_argument(root, &page))?;
    let explanations = explain_page(&state, &page, line)?;
    finish(state)?;
    if options.json {
        io.out(&pretty(&to_json(&page, &explanations)));
    } else {
        io.out(&format(&page, &explanations));
    }
    Ok(0)
}

/// `<page>:<line>`, or a page alone. A line counts from 1.
fn parse_target(target: &str) -> Result<(String, Option<usize>)> {
    let Some((page, line)) =
        target.rsplit_once(':').filter(|(_, l)| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()))
    else {
        return Ok((target.to_string(), None));
    };
    match line.parse::<usize>() {
        Ok(line) if line > 0 => Ok((page.to_string(), Some(line))),
        _ => Err(Error::user(format!("\"{target}\" names no line: lines count from 1; {USAGE}"))),
    }
}

/// The one living page a path names, which `pages` lists: a source file holds doc-comment
/// examples, never directives.
fn one_page(state: &State, given: &str) -> Result<String> {
    let mut selected = select_pages(&[given.to_string()], &state.living, &state.frozen)?.into_iter();
    let (Some(page), None) = (selected.next(), selected.next()) else {
        return Err(Error::user(format!("\"{given}\" matches several pages; explain takes one")));
    };
    if !matcher(&state.config.pages)?.is_match(&page) {
        return Err(Error::user(format!(
            "{page} is a source file: its doc comments hold examples, not directives"
        )));
    }
    Ok(page)
}

/// Each directive on `page`, or on its `line`, explained, in line order. The page is planned
/// again for why each directive is what it is; its attestations are taken from the project's
/// plan, which also judged what only the project can tell (a cited file's language, say).
fn explain_page(state: &State, page: &str, line: Option<usize>) -> Result<Vec<Explanation>> {
    let explained = plan_again(state, page)?;
    let directives: Vec<(&Attestation, Why)> = state
        .attestations
        .iter()
        .filter(|a| a.page == page)
        .filter_map(|a| Some((a, *explained.whys.get(&a.id)?)))
        .collect();
    let declared = explained.aliases.declared();
    let at = |written: usize| line.is_none_or(|line| line == written);
    let aliases: Vec<&(Alias, usize)> = declared.iter().filter(|(_, written)| at(*written)).collect();
    let chosen: Vec<(&Attestation, Why)> = directives.iter().copied().filter(|(a, _)| at(a.line)).collect();
    if chosen.is_empty() && aliases.is_empty() {
        let lines =
            directives.iter().map(|(a, _)| a.line).chain(declared.iter().map(|(_, written)| *written));
        return Err(no_directive(page, line, lines.collect()));
    }
    let on_page: Vec<&Attestation> = directives.iter().map(|(a, _)| *a).collect();
    let attestations: Vec<&Attestation> = chosen.iter().map(|(a, _)| *a).collect();
    let results = results_of(state, network(), &attestations)?;
    let attesting = chosen.into_iter().zip(results).map(|((a, why), results)| Explanation::Attests {
        attestation: Box::new(a.clone()),
        why,
        results,
    });
    let named = aliases.into_iter().map(|(alias, written)| Explanation::of_alias(alias, *written, &on_page));
    let mut explanations: Vec<Explanation> = named.chain(attesting).collect();
    explanations.sort_by_key(Explanation::line);
    Ok(explanations)
}

fn plan_again(state: &State, page: &str) -> Result<Explained> {
    let text = state.files.read(page).map_err(Error::user)?;
    let parsed = parse_page(&text).map_err(|why| why.on(page))?;
    Ok(plan_explained(page, &parsed, &example_options(&state.config, &state.adapter)))
}

/// The error for a page, or a line of one, that holds no directive, with the `lines` that do.
fn no_directive(page: &str, line: Option<usize>, mut lines: Vec<usize>) -> Error {
    let Some(line) = line else { return Error::user(format!("{page} holds no directive")) };
    if lines.is_empty() {
        return Error::user(format!("{page}:{line} holds no directive, and no line of the page does"));
    }
    lines.sort_unstable();
    lines.dedup();
    let shown: Vec<String> = lines.iter().map(|line| format!("L{line}")).collect();
    Error::user(format!("{page}:{line} holds no directive; directives are at {}", shown.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_a_page_and_perhaps_a_line_that_counts_from_one() {
        assert_eq!(parse_target("docs/a.md:12"), Ok(("docs/a.md".into(), Some(12))));
        assert_eq!(parse_target("docs/a.md"), Ok(("docs/a.md".into(), None)));
        assert_eq!(parse_target(r"C:\docs\a.md"), Ok((r"C:\docs\a.md".into(), None)));
        assert_eq!(parse_target("docs/a:b.md:3"), Ok(("docs/a:b.md".into(), Some(3))));
        let zero = parse_target("docs/a.md:0").unwrap_err().to_string();
        assert_eq!(zero, format!("docattest: \"docs/a.md:0\" names no line: lines count from 1; {USAGE}"));
    }

    #[test]
    fn a_line_with_no_directive_names_the_lines_that_hold_one() {
        let said = no_directive("docs/a.md", Some(4), vec![7, 3, 7]).to_string();
        assert_eq!(said, "docattest: docs/a.md:4 holds no directive; directives are at L3, L7");
        let empty = no_directive("docs/a.md", Some(4), Vec::new()).to_string();
        assert_eq!(empty, "docattest: docs/a.md:4 holds no directive, and no line of the page does");
        assert_eq!(
            no_directive("docs/a.md", None, Vec::new()).to_string(),
            "docattest: docs/a.md holds no directive"
        );
    }
}
