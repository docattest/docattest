//! Manifests: the project files a fact reads with `manifest("<path>")`, such as a package's
//! manifest or a config file, parsed by their format and given to the adapter when the
//! session opens, as snapshots are (`src/manifests/`). The core reads formats (JSON, TOML,
//! `go.mod`); what a field of a manifest means is the page's to say.
//!
//! Only the paths facts write out as a string literal are read. The core never evaluates an
//! expression, and the adapter cannot ask the core for a file while it evaluates one, so the
//! session carries every manifest a fact may read, and a path built at run time is not one.

mod format;
mod go_mod;

use std::path::{Component, Path};
use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::api::attestation::{Attestation, Body};
use crate::files::project_files::ProjectFiles;
use crate::files::source::{normalise, posix_normalize};
use format::parse_manifest;

/// One manifest the facts name: the path as a fact writes it, the project file it is (None
/// when the path leaves the project), and its data or why there is none.
#[derive(Debug, Clone, PartialEq)]
struct Named {
    written: String,
    file: Option<String>,
    data: Result<Value, String>,
}

/// Every manifest the facts of a project name, each once, in the order they are first named.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Manifests {
    named: Vec<Named>,
}

/// Read and parse every manifest the facts in `attestations` name.
pub fn load_manifests(files: &dyn ProjectFiles, attestations: &[Attestation]) -> Manifests {
    let mut named: Vec<Named> = Vec::new();
    for written in attestations.iter().filter_map(expression_of).flat_map(named_in) {
        if !named.iter().any(|n| n.written == written) {
            named.push(read(files, written));
        }
    }
    Manifests { named }
}

/// How a page names the manifest at `file`: the call a fact makes.
pub fn manifest_call(file: &str) -> String {
    format!("manifest(\"{file}\")")
}

impl Manifests {
    /// The data of every manifest that could be read, by the path as facts write it: what
    /// `open` gives the adapter.
    pub fn to_json(&self) -> Value {
        let read = self.named.iter().filter_map(|n| Some((n.written.clone(), n.data.as_ref().ok()?.clone())));
        Value::Object(read.collect::<Map<String, Value>>())
    }

    /// Make each fact that names a manifest that cannot be read an `error` that never runs,
    /// saying which file and why.
    pub fn refuse_unreadable(&self, attestations: &mut [Attestation]) {
        for a in attestations.iter_mut().filter(|a| a.ok()) {
            let Some(expression) = expression_of(a) else { continue };
            let unreadable = named_in(expression).find_map(|written| self.problem(written));
            if let Some(problem) = unreadable {
                a.error = Some(problem.to_string());
            }
        }
    }

    /// The facts that read a manifest among `changed` (project-relative files), each with
    /// that file: a change to the file can make them untrue.
    pub fn readers<'a>(
        &self,
        attestations: &'a [Attestation],
        changed: &[String],
    ) -> Vec<(String, &'a Attestation)> {
        let mut out: Vec<(String, &Attestation)> = Vec::new();
        for a in attestations {
            let Some(expression) = expression_of(a) else { continue };
            for file in named_in(expression).filter_map(|written| self.file(written)) {
                let reads = |(f, r): &(String, &Attestation)| f == file && r.id == a.id;
                if changed.iter().any(|c| c == file) && !out.iter().any(reads) {
                    out.push((file.to_string(), a));
                }
            }
        }
        out
    }

    fn problem(&self, written: &str) -> Option<&str> {
        self.named(written)?.data.as_ref().err().map(String::as_str)
    }

    fn file(&self, written: &str) -> Option<&str> {
        self.named(written)?.file.as_deref()
    }

    /// The manifest a fact names as `written`.
    fn named(&self, written: &str) -> Option<&Named> {
        self.named.iter().find(|n| n.written == written)
    }
}

fn expression_of(attestation: &Attestation) -> Option<&str> {
    match &attestation.body {
        Body::Fact(fact) => Some(&fact.expression),
        _ => None,
    }
}

/// The paths an expression passes to `manifest` as a string literal, in any of the quotes
/// the adapters' languages write one with. A `manifest` that is a member of something else
/// (`x.manifest("…")`) is not the one docattest gives.
fn named_in(expression: &str) -> impl Iterator<Item = &str> {
    static CALL: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let call = CALL.get_or_init(|| {
        fancy_regex::Regex::new(r#"(?<![\w$.])manifest\(\s*(["'`])([^"'`\\\r\n]*)\1\s*\)"#)
            .expect("the pattern of a manifest call is valid")
    });
    call.captures_iter(expression).filter_map(|c| Some(c.ok()?.get(2)?.as_str()))
}

fn read(files: &dyn ProjectFiles, written: &str) -> Named {
    let file = inside_project(written);
    let data = match &file {
        None => Err(format!("manifest \"{written}\" is outside the project")),
        Some(file) if !files.exists(file) => Err(format!("manifest \"{written}\": no such file")),
        Some(file) => files
            .read(file)
            .and_then(|text| parse_manifest(file, &text))
            .map_err(|why| format!("manifest \"{written}\": {why}")),
    };
    Named { written: written.to_string(), file, data }
}

/// The project-relative file a path names, or None when it leaves the project: it is
/// absolute, or climbs above the root.
fn inside_project(written: &str) -> Option<String> {
    let file = posix_normalize(&normalise(written));
    let inside = Path::new(&file).components().all(|c| matches!(c, Component::Normal(_)));
    inside.then_some(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::memory::MemoryFiles;
    use crate::project::plan::plan_project;
    use serde_json::json;

    fn named(expression: &str) -> Vec<&str> {
        named_in(expression).collect()
    }

    #[test]
    fn a_manifest_is_named_by_a_literal_path_in_any_quotes() {
        assert_eq!(named(r#"manifest("package.json").engines.node"#), ["package.json"]);
        assert_eq!(named("manifest('pyproject.toml')['project']"), ["pyproject.toml"]);
        assert_eq!(named("manifest( `go.mod` ).go + manifest(\"a/b.json\")"), ["go.mod", "a/b.json"]);
        assert_eq!(named("manifest(path)"), Vec::<&str>::new());
        assert_eq!(named(r#"loader.manifest("x.json")"#), Vec::<&str>::new());
        assert_eq!(named(r#"my_manifest("x.json")"#), Vec::<&str>::new());
        // Quotes that do not match name nothing. The apostrophe is written \u{27}: a bare one
        // in a string reads as the start of a character to tools that measure this file.
        assert_eq!(named("manifest(\"x.json\u{27} )"), Vec::<&str>::new());
    }

    #[test]
    fn a_path_must_stay_inside_the_project() {
        assert_eq!(inside_project("./a//b.json"), Some("a/b.json".to_string()));
        assert_eq!(inside_project("a/../b.json"), Some("b.json".to_string()));
        assert_eq!(inside_project("../b.json"), None);
        assert_eq!(inside_project("a/../../b.json"), None);
        assert_eq!(inside_project("/etc/b.json"), None);
        assert_eq!(inside_project(""), None);
    }

    fn project(page: &str, extra: &[(&str, &str)]) -> (MemoryFiles, Vec<Attestation>) {
        let mut files = vec![
            (
                "docattest.config.json".to_string(),
                r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"typescript"}"#.to_string(),
            ),
            ("docs/a.md".to_string(), page.to_string()),
        ];
        files.extend(extra.iter().map(|(path, text)| (path.to_string(), text.to_string())));
        let files = MemoryFiles::new(files);
        let config = crate::config::parse_config(&files.read("docattest.config.json").unwrap()).unwrap();
        let plan = plan_project(&files, &config, &crate::adapter::fake::typescript()).unwrap();
        (files, plan.attestations)
    }

    #[test]
    fn the_session_carries_what_could_be_read_and_each_fact_over_the_rest_is_an_error() {
        let page = [
            "Node `22` <!-- fact: manifest(\"package.json\").engines.node -->",
            "",
            "Go `1.23` <!-- fact: manifest('./go.mod').go -->",
            "",
            "Gone `1` <!-- fact: manifest(\"gone.json\").version -->",
            "",
            "Out `1` <!-- fact: manifest(\"../up.json\").version -->",
            "",
        ]
        .join("\n");
        let extra = [("package.json", r#"{"engines":{"node":"22"}}"#), ("go.mod", "module m\n\ngo 1.23\n")];
        let (files, mut attestations) = project(&page, &extra);
        let manifests = load_manifests(&files, &attestations);
        assert_eq!(
            manifests.to_json(),
            json!({ "package.json": { "engines": { "node": "22" } }, "./go.mod": { "module": "m", "go": "1.23" } })
        );
        manifests.refuse_unreadable(&mut attestations);
        let errors: Vec<Option<&str>> = attestations.iter().map(Attestation::problem).collect();
        assert_eq!(
            errors,
            [
                None,
                None,
                Some("manifest \"gone.json\": no such file"),
                Some("manifest \"../up.json\" is outside the project"),
            ]
        );
    }

    #[test]
    fn the_readers_of_a_changed_manifest_are_the_facts_that_name_it() {
        let page = "A `1` <!-- fact: manifest(\"./p.json\").a -->\n\nB `2` <!-- fact: manifest(\"p.json\").b -->\n\nC `3` <!-- fact: manifest(\"q.json\").c -->\n";
        let extra = [("p.json", r#"{"a":1,"b":2}"#), ("q.json", r#"{"c":3}"#)];
        let (files, attestations) = project(page, &extra);
        let manifests = load_manifests(&files, &attestations);
        let readers = manifests.readers(&attestations, &["p.json".to_string()]);
        let lines: Vec<(&str, usize)> = readers.iter().map(|(file, a)| (file.as_str(), a.line)).collect();
        assert_eq!(lines, [("p.json", 1), ("p.json", 3)]);
        assert_eq!(manifest_call("p.json"), r#"manifest("p.json")"#);
    }
}
