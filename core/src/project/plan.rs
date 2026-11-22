//! Planning a project: which pages are living, and every attestation on them. Nothing runs;
//! the language says which `check` blocks assert, and what the doc comments of the sources
//! hold. The working tree and the base of a `check --base` are planned the same way.

use std::collections::{HashMap, HashSet};

use crate::adapter::Language;
use crate::api::attestation::{Attestation, Body, Kind, Test};
use crate::api::errors::{Error, Result};
use crate::attestations::examples::{ExampleOptions, checks, mark_weak};
use crate::attestations::plan::plan_parsed;
use crate::config::{ProjectConfig, matcher};
use crate::counted::counted;
use crate::files::glob::GlobMatcher;
use crate::files::project_files::ProjectFiles;
use crate::markdown::model::{Declared, Page};
use crate::markdown::parse::parse_page_for;
use crate::project::config_warnings::config_warnings;

/// The pages, sorted into living and frozen, and the attestations planned on the living.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Matched by `pages` and not frozen, then the sources whose doc comments hold examples.
    pub living: Vec<String>,
    /// The living pages marked as prose, by `prose` or by their frontmatter, that carry no
    /// attestation: they have nothing to prove, so they are not uncovered.
    pub prose: Vec<String>,
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
    let options = example_options(config, language);
    let mut plan = Plan::of_pages(&pages, parsed, &options, &matcher(&config.prose)?);
    // A citation is checked against what the adapter reads in the cited file. One that
    // cannot read tests leaves every citation unproven, and the page should say so.
    if !language.reads_tests() {
        plan.refuse_citations(language.name());
    }
    plan.refuse_aliases_that_are_files(files);
    let sources = source_texts(files, &all, config, language)?;
    plan.ask_language(language, &sources)?;
    let stale_prose = std::mem::take(&mut plan.warnings);
    plan.warnings = [config_warnings(files, &all, config, &plan.attestations)?, stale_prose].concat();
    Ok(plan)
}

/// What planning a page needs to know of the config and the language.
pub(crate) fn example_options<'a>(
    config: &'a ProjectConfig,
    language: &'a dyn Language,
) -> ExampleOptions<'a> {
    ExampleOptions {
        languages: language.block_languages(),
        ignore_tags: &config.ignore_tags,
        info_words: language.info_words(),
    }
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
        parse_page_for(page, &text).map(Some).map_err(|why| why.on(page))
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

/// How a page is marked as prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProseMarker {
    Config,
    Frontmatter,
}

impl ProseMarker {
    /// How a page is marked, if it is: the config's `prose` first, then its frontmatter.
    fn of(page: &str, parsed: &Page, prose: &GlobMatcher) -> Option<ProseMarker> {
        if prose.is_match(page) {
            return Some(ProseMarker::Config);
        }
        (parsed.declared == Declared::Prose).then_some(ProseMarker::Frontmatter)
    }

    fn removal(self) -> &'static str {
        match self {
            ProseMarker::Config => "take it out of \"prose\" in the config",
            ProseMarker::Frontmatter => "drop \"docattest: prose\" from its frontmatter",
        }
    }
}

impl Plan {
    fn of_pages(
        pages: &[&String],
        parsed: Vec<Option<Page>>,
        options: &ExampleOptions,
        prose: &GlobMatcher,
    ) -> Plan {
        let mut plan = Plan::default();
        for (page, parsed) in pages.iter().zip(parsed) {
            match parsed {
                Some(parsed) if !parsed.is_frozen() => {
                    plan.living.push((*page).clone());
                    let planned = plan_parsed(page, &parsed, options);
                    if let Some(marker) = ProseMarker::of(page, &parsed, prose) {
                        plan.mark_prose(page, marker, planned.len());
                    }
                    plan.attestations.extend(planned);
                }
                _ => plan.frozen.push((*page).clone()),
            }
        }
        plan
    }

    /// A page marked as prose is prose while it carries no attestation. Once it gains one it
    /// is covered like any other page, and the marker, which now says something false about
    /// it, is a warning.
    fn mark_prose(&mut self, page: &str, marker: ProseMarker, attestations: usize) {
        if attestations == 0 {
            self.prose.push(page.to_string());
            return;
        }
        let has = counted(attestations, "attestation");
        let removal = marker.removal();
        self.warnings.push(format!("{page} is marked as prose but has {has}: {removal}"));
    }

    fn refuse_citations(&mut self, adapter: &str) {
        for a in self.attestations.iter_mut().filter(|a| a.kind() == Kind::Test && a.ok()) {
            a.error = Some(format!(
                "test at L{} cannot be cited: the {adapter} adapter does not read test files",
                a.line
            ));
        }
    }

    /// A citation that names its file by an alias must not name a file of the project too:
    /// which of the two it cites would be anyone's guess.
    fn refuse_aliases_that_are_files(&mut self, files: &dyn ProjectFiles) {
        for a in self.attestations.iter_mut().filter(|a| a.ok()) {
            if let Body::Test(Test { alias: Some(name), .. }) = &a.body
                && files.exists(name)
            {
                a.error = Some(format!(
                    "test at L{} names {name}, which is both an alias on its page and a file of the \
                     project: rename the alias",
                    a.line
                ));
            }
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
    fn a_page_marked_as_prose_is_living_and_its_marker_goes_stale_with_a_proof() {
        let config = parse_config(
            r#"{"pages":["docs/*.md"],"prose":["docs/guide*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#,
        )
        .unwrap();
        let fact = "N is `3`. <!-- fact: N -->\n";
        let files = crate::files::memory::MemoryFiles::new(
            [
                ("docs/guide.md", "# Guide\n"),
                ("docs/guide-old.md", fact),
                ("docs/notes.md", "---\ndocattest: prose\n---\n# Notes\n"),
                ("docs/howto.md", &format!("---\ndocattest: prose\n---\n{fact}")),
                ("docs/plain.md", "Prose.\n"),
            ]
            .map(|(page, text)| (page.to_string(), text.to_string())),
        );
        let plan = plan_project(&files, &config, &typescript()).unwrap();
        assert_eq!(plan.living.len(), 5);
        assert_eq!(plan.prose, ["docs/guide.md", "docs/notes.md"]);
        assert_eq!(
            plan.warnings,
            [
                "docs/guide-old.md is marked as prose but has 1 attestation: take it out of \"prose\" in the config",
                "docs/howto.md is marked as prose but has 1 attestation: drop \"docattest: prose\" from its frontmatter",
            ]
        );
    }

    #[test]
    fn an_alias_named_like_a_file_of_the_project_is_an_error_where_it_is_cited() {
        let config =
            parse_config(r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#)
                .unwrap();
        let page = "<!-- tests: launch = t/launch.test.ts -->\n\nIt runs. <!-- test: launch > \"runs\" -->\n";
        let errors = |extra: &[&str]| {
            let listed = [("docs/a.md", page), ("t/launch.test.ts", "")]
                .into_iter()
                .chain(extra.iter().map(|f| (*f, "")));
            let files =
                crate::files::memory::MemoryFiles::new(listed.map(|(f, t)| (f.to_string(), t.to_string())));
            let plan = plan_project(&files, &config, &typescript()).unwrap();
            plan.attestations.iter().map(|a| a.problem().map(String::from)).collect::<Vec<_>>()
        };
        assert_eq!(errors(&[]), [None]);
        assert_eq!(
            errors(&["launch"]),
            [Some("test at L3 names launch, which is both an alias on its page and a file of the project: rename the alias".into())]
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
