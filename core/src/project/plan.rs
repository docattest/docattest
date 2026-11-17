//! Planning a project: which pages are living, and every attestation on them. Nothing runs;
//! the language says which `check` blocks assert, and what the doc comments of the sources
//! hold. The working tree and the base of a `check --base` are planned the same way.

use std::collections::{HashMap, HashSet};

use crate::adapter::Language;
use crate::api::attestation::{Attestation, Kind};
use crate::api::errors::{Error, Result};
use crate::attestations::examples::{ExampleOptions, checks, mark_weak};
use crate::attestations::plan::plan_parsed;
use crate::config::{ProjectConfig, matcher};
use crate::files::glob::GlobMatcher;
use crate::files::project_files::ProjectFiles;
use crate::markdown::model::Page;
use crate::markdown::parse::parse_page;
use crate::project::config_warnings::config_warnings;

/// The pages, sorted into living and frozen, and the attestations planned on the living.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Matched by `pages` and not frozen, then the sources whose doc comments hold examples.
    pub living: Vec<String>,
    pub frozen: Vec<String>,
    /// Every attestation on every living page, in plan order.
    pub attestations: Vec<Attestation>,
    /// What the config names that does nothing in the project.
    pub warnings: Vec<String>,
}

/// Plan every page `config` names in `files`, and the examples in the sources' doc comments.
pub fn plan_project(
    files: &dyn ProjectFiles,
    config: &ProjectConfig,
    language: &dyn Language,
) -> Result<Plan> {
    let all = files.list();
    let is_page = matcher(&config.pages)?;
    let pages: Vec<&String> = all.iter().filter(|f| is_page.is_match(f)).collect();
    let parsed = parse_pages(files, &pages, &matcher(&config.frozen)?)?;
    crate::trace::mark("pages parsed");
    let options = ExampleOptions {
        languages: language.block_languages(),
        ignore_tags: &config.ignore_tags,
        info_words: language.info_words(),
    };
    let mut plan = Plan::of_pages(&pages, parsed, &options);
    // A citation is checked against what the adapter reads in the cited file. One that
    // cannot read tests leaves every citation unproven, and the page should say so.
    if !language.reads_tests() {
        plan.refuse_citations(language.name());
    }
    let sources = source_texts(files, &all, config, language)?;
    plan.ask_language(language, &sources)?;
    plan.warnings = config_warnings(files, &all, config, &plan.attestations)?;
    Ok(plan)
}

/// Parse every page that is not frozen by glob, in parallel; None for those frozen pages.
/// The first page that cannot be parsed, in page order, fails the plan.
fn parse_pages(
    files: &dyn ProjectFiles,
    pages: &[&String],
    frozen: &GlobMatcher,
) -> Result<Vec<Option<Page>>> {
    let parse = |page: &&String| -> Result<Option<Page>> {
        if frozen.is_match(page) {
            return Ok(None);
        }
        let text = files.read(page).map_err(Error::user)?;
        parse_page(&text).map(Some).map_err(|why| why.on(page))
    };
    crate::parallel::map_in_parallel(pages, parse).into_iter().collect()
}

/// The files "sources" names that the language calls source files, with their text.
fn source_texts(
    files: &dyn ProjectFiles,
    all: &[String],
    config: &ProjectConfig,
    language: &dyn Language,
) -> Result<Vec<(String, String)>> {
    let is_source = matcher(&config.sources)?;
    all.iter()
        .filter(|f| is_source.is_match(f) && language.source_files().is_match(f))
        .map(|f| Ok((f.clone(), files.read(f).map_err(Error::user)?)))
        .collect()
}

impl Plan {
    fn of_pages(pages: &[&String], parsed: Vec<Option<Page>>, options: &ExampleOptions) -> Plan {
        let mut plan = Plan::default();
        for (page, parsed) in pages.iter().zip(parsed) {
            match parsed {
                Some(parsed) if !parsed.frozen => {
                    plan.living.push((*page).clone());
                    plan.attestations.extend(plan_parsed(page, &parsed, options));
                }
                _ => plan.frozen.push((*page).clone()),
            }
        }
        plan
    }

    fn refuse_citations(&mut self, adapter: &str) {
        for a in self.attestations.iter_mut().filter(|a| a.kind() == Kind::Test && a.ok()) {
            a.error = Some(format!(
                "test at L{} cannot be cited: the {adapter} adapter does not read test files",
                a.line
            ));
        }
    }

    /// Which `check` blocks assert anything, and the examples in the sources' doc comments.
    fn ask_language(&mut self, language: &dyn Language, sources: &[(String, String)]) -> Result<()> {
        let texts: Vec<(&str, &str)> =
            sources.iter().map(|(page, text)| (page.as_str(), text.as_str())).collect();
        let (asserting, examples) = {
            let asked = checks(&self.attestations);
            language.assertions_and_doc_comments(&asked, &texts)?
        };
        if !asserting.is_empty() {
            mark_weak(&mut self.attestations, &asserting);
        }
        self.add_examples(examples, sources);
        Ok(())
    }

    /// The doc comments' examples: each source that has any becomes a living page, after
    /// the pages, in the order of the sources.
    fn add_examples(&mut self, examples: Vec<Attestation>, sources: &[(String, String)]) {
        let mut by_page: HashMap<String, Vec<Attestation>> = HashMap::new();
        for a in examples {
            by_page.entry(a.page.clone()).or_default().push(a);
        }
        let mut living: HashSet<String> = self.living.iter().cloned().collect();
        for (source, _) in sources {
            let Some(found) = by_page.remove(source) else { continue };
            if living.insert(source.clone()) {
                self.living.push(source.clone());
            }
            self.attestations.extend(found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::fake::typescript;
    use crate::config::parse_config;

    /// A project whose one page is listed but cannot be read.
    struct Unreadable;

    impl ProjectFiles for Unreadable {
        fn read(&self, file: &str) -> std::result::Result<String, String> {
            Err(format!("cannot read {file}: permission denied"))
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
        fn list(&self) -> Vec<String> {
            vec!["docs/a.md".to_string()]
        }
    }

    #[test]
    fn a_page_that_cannot_be_read_is_the_users_to_fix_not_an_empty_page() {
        let config =
            parse_config(r#"{"pages":["docs/**/*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#)
                .unwrap();
        let planned = plan_project(&Unreadable, &config, &typescript());
        assert_eq!(
            planned.err(),
            Some(Error::User("docattest: cannot read docs/a.md: permission denied".into()))
        );
    }

    #[test]
    fn a_page_the_parser_cannot_place_is_named_in_the_internal_error() {
        let config =
            parse_config(r#"{"pages":["docs/**/*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#)
                .unwrap();
        let files = crate::files::memory::MemoryFiles::new(vec![
            ("docs/a.md".to_string(), "# A\n".to_string()),
            ("docs/b.md".to_string(), "say 'www.example.com' ok\n".to_string()),
        ]);
        let planned = plan_project(&files, &config, &typescript());
        assert_eq!(planned.err(), Some(Error::internal("docs/b.md: markdown node \"text\" has no position")));
    }
}
