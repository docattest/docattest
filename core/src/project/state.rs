//! Everything parsing a project yields, before anything runs.

use std::collections::HashSet;

use crate::adapter::{Adapter, Launch, Start, Starting};
use crate::api::attestation::Attestation;
use crate::api::errors::{Error, Result};
use crate::config::{ProjectConfig, load_config};
use crate::files::glob::{GlobMatcher, is_glob};
use crate::files::source::{DiskFiles, normalise};
use crate::manifests::{Manifests, load_manifests};
use crate::project::launches::Launches;
use crate::project::plan::{Plan, plan_project};
use crate::snapshots::{LoadedSnapshots, load_snapshots};

/// Everything parsing a project yields: no execution has happened yet.
pub struct State {
    pub files: DiskFiles,
    pub config: ProjectConfig,
    pub adapter: Adapter,
    /// Living pages: matched by `pages`, not frozen, plus sources with doc-comment examples.
    pub living: Vec<String>,
    pub frozen: Vec<String>,
    /// Every attestation on every living page.
    pub attestations: Vec<Attestation>,
    pub snapshots: LoadedSnapshots,
    /// The manifests the facts name.
    pub manifests: Manifests,
    /// The instant the project was loaded at, in ms since the epoch: the snapshots' age is
    /// counted to it, and a sample page's fixture is loaded at it too.
    pub now: f64,
    /// How its adapter and its members' were started, so a docattest run inside its check
    /// (a sample page's fixture, a console block) starts the same ones.
    pub launches: Launches,
}

/// A project whose config was read and whose adapter was started: `hello` is on its way,
/// nothing else has been asked and no page has been read.
pub struct Started {
    pub files: DiskFiles,
    pub config: ProjectConfig,
    pub adapter: Starting,
    pub launch: Launch,
}

/// A project whose adapter has greeted: what it said of its language is known, and no page
/// has been read.
pub struct Greeted {
    pub files: DiskFiles,
    pub config: ProjectConfig,
    pub adapter: Adapter,
    pub launch: Launch,
}

/// Read the config and start the adapter. A project without a valid config, or without an
/// adapter to start, fails here, before anything else is done.
pub fn start(files: DiskFiles, start: Start) -> Result<Started> {
    launch(files, &Launch::Found, start)
}

/// The same, with the adapter started as `launch` says.
pub fn launch(files: DiskFiles, launch: &Launch, start: Start) -> Result<Started> {
    let config = load_config(&files)?;
    started(files, config, launch.clone(), start)
}

/// Read the config, start the adapter, find every page and plan every attestation.
pub fn load_state(files: DiskFiles, now: f64) -> Result<State> {
    start(files, Start::Warm)?.load(now)
}

/// [`load_state`] for a project beside one that is being checked (a sample page's fixture):
/// its adapter starts as the checked project, or one of its members, started an adapter of
/// that language, since the same launcher or `members` entry is what found it.
pub fn load_state_beside(files: DiskFiles, now: f64, launches: &Launches) -> Result<State> {
    let config = load_config(&files)?;
    let launch = launches.for_language(&config.adapter);
    started(files, config, launch, Start::Warm)?.load(now)
}

fn started(files: DiskFiles, config: ProjectConfig, launch: Launch, start: Start) -> Result<Started> {
    let adapter = Adapter::launch(&config.adapter, &launch, start)?;
    Ok(Started { files, config, adapter, launch })
}

impl Started {
    /// Wait for the adapter's greeting. Which names are never the project's own is the
    /// adapter's to say, so the greeting comes before the project is listed. It greets
    /// without loading its compiler, and loads that while the pages are read and parsed.
    pub fn greet(self) -> Result<Greeted> {
        let Started { files, config, adapter, launch } = self;
        Ok(Greeted { files, config, adapter: adapter.greeted()?, launch })
    }

    /// Greet, then find every page and plan every attestation.
    pub fn load(self, now: f64) -> Result<State> {
        self.greet()?.load(now)
    }
}

impl Greeted {
    /// Find every page and plan every attestation.
    pub fn load(self, now: f64) -> Result<State> {
        let Greeted { mut files, config, adapter, launch } = self;
        files.ignore(&adapter.ignored);
        let Plan { living, frozen, mut attestations } = plan_project(&files, &config, &adapter)?;
        let snapshots = load_snapshots(&files, config.snapshots_dir.as_deref(), now, config.max_age_days);
        let manifests = load_manifests(&files, &attestations);
        manifests.refuse_unreadable(&mut attestations);
        let launches = Launches::of(&files, &config, launch);
        let state = State {
            files,
            config,
            adapter,
            living,
            frozen,
            attestations,
            snapshots,
            manifests,
            now,
            launches,
        };
        Ok(state)
    }
}

/// The living pages a list of paths or globs names. Every pattern must match a living page,
/// so a typo fails instead of checking nothing.
pub fn select_pages(patterns: &[String], living: &[String], frozen: &[String]) -> Result<HashSet<String>> {
    let mut selected = HashSet::new();
    for pattern in patterns {
        let wanted = normalise(pattern);
        let glob = if is_glob(&wanted) {
            let why = |why: String| Error::user(format!("\"{wanted}\" is a glob that cannot be read: {why}"));
            Some(GlobMatcher::new(std::slice::from_ref(&wanted)).map_err(why)?)
        } else {
            None
        };
        let matches = |page: &String| glob.as_ref().map_or(*page == wanted, |g| g.is_match(page));
        let hits: Vec<&String> = living.iter().filter(|p| matches(p)).collect();
        if hits.is_empty() {
            if frozen.iter().any(matches) {
                return Err(Error::user(format!(
                    "\"{pattern}\" matches only frozen pages, which are never checked"
                )));
            }
            let hint = "pages come from \"pages\" in docattest.config.json, minus \"frozen\"";
            return Err(Error::user(format!("\"{pattern}\" matches no living page ({hint})")));
        }
        selected.extend(hits.into_iter().cloned());
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    /// An adapter that greets with `hello` and answers nothing else.
    fn greeting(project: &Temp, hello: &str) -> Starting {
        let answer = format!(r#"{{"id":1,"result":{hello}}}"#);
        project.write("adapter.sh", &format!("read hello\nprintf '%s\\n' '{answer}'\ncat >/dev/null\n"));
        let script = project.0.join("adapter.sh").to_string_lossy().into_owned();
        Adapter::start_command(&["sh".to_string(), script], "fake", Start::Cold).unwrap()
    }

    fn loaded(project: &Temp, hello: &str) -> State {
        let files = DiskFiles::new(project.0.clone());
        let config = load_config(&files).unwrap();
        Started { files, config, adapter: greeting(project, hello), launch: Launch::Found }.load(0.0).unwrap()
    }

    const HELLO: &str = r#"{"protocol":1,"adapter":{"name":"fake","version":"0"},"blockLanguages":["fake"],"sourceFiles":{"include":[],"exclude":[]},"ignoredNames":["*.out"]"#;

    #[test]
    fn a_citation_needs_an_adapter_that_reads_tests() {
        let project = Temp::new("cite");
        project.write(
            "docattest.config.json",
            r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"fake"}"#,
        );
        project.write("docs/a.md", "It retries. <!-- test: t/a.fake > \"retries\" -->\n");
        let errors = |state: &State| -> Vec<Option<String>> {
            state.attestations.iter().map(|a| a.problem().map(String::from)).collect()
        };
        let without = loaded(&project, &format!("{HELLO},\"capabilities\":[]}}"));
        assert_eq!(
            errors(&without),
            [Some("test at L1 cannot be cited: the fake adapter does not read test files".to_string())]
        );
        let with = loaded(&project, &format!("{HELLO},\"capabilities\":[\"tests\"]}}"));
        assert_eq!(errors(&with), [None]);
    }

    #[test]
    fn the_greeting_says_what_is_ignored_and_which_words_are_its_own() {
        let project = Temp::new("greeted");
        project.write(
            "docattest.config.json",
            r#"{"pages":["**/*.md"],"surfaces":{"default":"x"},"adapter":"fake"}"#,
        );
        for page in ["docs/a.md", "build.out/b.md", "env/c.md", "env/marker.cfg"] {
            project.write(page, "```fake compile again title=\"x\"\ncode\n```\n");
        }
        let hello = format!(
            "{HELLO},\"capabilities\":[],\"ignoredMarkers\":[\"marker.cfg\"],\"infoWords\":[\"again\"]}}"
        );
        let state = loaded(&project, &hello);
        assert_eq!(state.living, ["docs/a.md"]);
        let tags: Vec<_> = state.attestations.iter().map(|a| a.to_json()["tags"].clone()).collect();
        assert_eq!(tags, [serde_json::json!(["again"])]);
        assert_eq!(state.attestations[0].error, None);
    }
}
