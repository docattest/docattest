//! What the config names that does nothing in the project: a glob of what to check that
//! matches no file, a setup module that is not there or that nothing reads, a glob of
//! symbols not meant to be documented that matches none the surface exports. Each is worth a
//! look, as a typo there checks less than was meant without a word, and none fails a check:
//! a project may name a page it has yet to write.

use crate::api::attestation::{Attestation, Kind};
use crate::api::errors::Result;
use crate::config::{ProjectConfig, matcher};
use crate::files::project_files::ProjectFiles;
use crate::files::source::{normalise, posix_normalize};

/// The warnings about `config` in a project whose files are `listed`, with `attestations`
/// planned on its living pages, in the order the config's keys are documented. A `frozen`
/// glob that matches nothing is no warning: it leaves nothing unchecked.
pub fn config_warnings(
    files: &dyn ProjectFiles,
    listed: &[String],
    config: &ProjectConfig,
    attestations: &[Attestation],
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    for (key, globs) in [("pages", &config.pages), ("sources", &config.sources)] {
        for glob in unmatched(globs, listed)? {
            warnings.push(format!("\"{key}\" has {glob:?}, which matches no file"));
        }
    }
    if let Some(setup) = &config.setup {
        warnings.extend(setup_warning(files, listed, setup, attestations));
    }
    Ok(warnings)
}

/// The warnings about the `undocumented` globs of `surface`, which exports `names`: one for
/// each glob that matches none of them. Only a surface's exports can tell, so `check` asks
/// once it has them.
pub fn undocumented_warnings(config: &ProjectConfig, surface: &str, names: &[String]) -> Result<Vec<String>> {
    let globs = unmatched(config.undocumented_globs(surface), names)?;
    let key = format!("undocumented.{surface}");
    Ok(globs.into_iter().map(|glob| format!("\"{key}\" has {glob:?}, which matches no symbol")).collect())
}

/// The globs that match none of `listed`, each matched on its own.
fn unmatched<'a>(globs: &'a [String], listed: &[String]) -> Result<Vec<&'a String>> {
    let mut unmatched = Vec::new();
    for glob in globs {
        let one = matcher(std::slice::from_ref(glob))?;
        if !listed.iter().any(|file| one.is_match(file)) {
            unmatched.push(glob);
        }
    }
    Ok(unmatched)
}

/// A setup module serves the cases tables, and the facts that call its helpers; which facts
/// do is the language's to say, so it is unused only when no living page has either.
fn setup_warning(
    files: &dyn ProjectFiles,
    listed: &[String],
    setup: &str,
    attestations: &[Attestation],
) -> Option<String> {
    let path = posix_normalize(&normalise(setup));
    let path = path.trim_end_matches('/');
    // A language may name its setup by a directory (a Go package) as well as by a file.
    let inside = format!("{path}/");
    if !files.exists(path) && !listed.iter().any(|file| file.starts_with(&inside)) {
        return Some(format!("\"setup\" is {setup:?}, which is not there"));
    }
    let reads_setup = |a: &Attestation| matches!(a.kind(), Kind::Cases | Kind::Fact);
    if attestations.iter().any(reads_setup) {
        return None;
    }
    Some(format!("\"setup\" is {setup:?}, which nothing reads: no living page has a cases table or a fact"))
}

#[cfg(test)]
mod tests {
    use super::undocumented_warnings;
    use crate::adapter::fake::typescript;
    use crate::config::parse_config;
    use crate::files::memory::MemoryFiles;
    use crate::project::plan::plan_project;

    fn warnings(config: &str, files: &[(&str, &str)]) -> Vec<String> {
        let files = MemoryFiles::new(files.iter().map(|(path, text)| (path.to_string(), text.to_string())));
        let config = parse_config(config).unwrap();
        plan_project(&files, &config, &typescript()).unwrap().warnings
    }

    const CASES: &str = "<!-- cases: total -->\n| a | total |\n|---|---|\n| 1 | 1 |\n";

    #[test]
    fn a_glob_that_matches_no_file_is_named_with_its_key() {
        let config = r#"{"pages":["docs/*.md","guide/**/*.md"],"frozen":["docs/old/**"],"sources":["src/**/*.ts","lib/**/*.ts"],"surfaces":{"default":"x"},"adapter":"typescript"}"#;
        assert_eq!(
            warnings(config, &[("docs/a.md", "# A\n"), ("src/index.ts", "")]),
            [
                "\"pages\" has \"guide/**/*.md\", which matches no file",
                "\"sources\" has \"lib/**/*.ts\", which matches no file",
            ]
        );
    }

    #[test]
    fn a_setup_module_must_be_there_and_be_read() {
        let config = |setup: &str| {
            format!(
                r#"{{"pages":["docs/*.md"],"setup":"{setup}","surfaces":{{"default":"x"}},"adapter":"typescript"}}"#
            )
        };
        let prose = [("docs/a.md", "Prose.\n"), ("docs/setup.ts", "")];
        assert_eq!(
            warnings(&config("docs/gone.ts"), &prose),
            ["\"setup\" is \"docs/gone.ts\", which is not there"]
        );
        assert_eq!(
            warnings(&config("./docs/setup.ts"), &prose),
            [
                "\"setup\" is \"./docs/setup.ts\", which nothing reads: no living page has a cases table or a fact"
            ]
        );
        let cases = [("docs/a.md", CASES), ("docs/setup.ts", "")];
        assert!(warnings(&config("docs/setup.ts"), &cases).is_empty());
        let fact = [("docs/a.md", "N is `3`. <!-- fact: N -->\n"), ("setup/cases.go", "")];
        assert!(warnings(&config("setup/"), &fact).is_empty());
    }

    #[test]
    fn a_glob_of_symbols_not_meant_to_be_documented_that_matches_none_is_named_with_its_surface() {
        let config = parse_config(
            r#"{"pages":[],"surfaces":{"default":"x","server":"y"},"adapter":"typescript","undocumented":{"default":["Program*","internal*"]}}"#,
        )
        .unwrap();
        let names = ["ProgramMain".to_string(), "Quote".to_string()];
        assert_eq!(
            undocumented_warnings(&config, "default", &names).unwrap(),
            ["\"undocumented.default\" has \"internal*\", which matches no symbol"]
        );
        assert!(undocumented_warnings(&config, "server", &names).unwrap().is_empty());
    }
}
