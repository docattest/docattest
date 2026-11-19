use super::*;
use crate::files::memory::MemoryFiles;
use crate::project::plan::plan_project;
use serde_json::json;

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
    let fact_files = load_fact_files(&files, &attestations);
    assert_eq!(
        Value::Object(fact_files.session_params()),
        json!({ "manifests": {
            "package.json": { "engines": { "node": "22" } }, "./go.mod": { "module": "m", "go": "1.23" }
        } })
    );
    fact_files.refuse_unreadable(&mut attestations);
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
fn a_file_is_read_as_its_text_and_by_the_same_rules() {
    let page = [
        "User `10001` <!-- fact: file(\"Dockerfile\").match(/USER (\\d+)/)[1] -->",
        "",
        "Name `x` <!-- fact: manifest(\"Dockerfile\").name -->",
        "",
        "Gone `1` <!-- fact: file(\"gone.txt\") -->",
        "",
        "Out `1` <!-- fact: file(\"../up.txt\") -->",
        "",
    ]
    .join("\n");
    let (files, mut attestations) = project(&page, &[("Dockerfile", "FROM scratch\nUSER 10001\n")]);
    let fact_files = load_fact_files(&files, &attestations);
    assert_eq!(
        Value::Object(fact_files.session_params()),
        json!({ "manifests": {}, "texts": { "Dockerfile": "FROM scratch\nUSER 10001\n" } })
    );
    fact_files.refuse_unreadable(&mut attestations);
    let errors: Vec<Option<&str>> = attestations.iter().map(Attestation::problem).collect();
    assert_eq!(
        errors,
        [
            None,
            Some(
                "manifest \"Dockerfile\": not a format docattest reads: JSON (.json), TOML (.toml), YAML (.yaml, .yml) or go.mod"
            ),
            Some("file \"gone.txt\": no such file"),
            Some("file \"../up.txt\" is outside the project"),
        ]
    );
}

/// A project of one file whose bytes are not UTF-8.
struct Binary;

impl ProjectFiles for Binary {
    fn read(&self, _: &str) -> std::result::Result<String, String> {
        unreachable!("a file a fact reads is read as bytes")
    }
    fn read_bytes(&self, _: &str) -> std::result::Result<Vec<u8>, String> {
        Ok(b"USER \xff\n".to_vec())
    }
    fn exists(&self, _: &str) -> bool {
        true
    }
    fn list(&self) -> Vec<String> {
        vec!["logo.png".to_string()]
    }
}

#[test]
fn a_file_that_is_not_utf8_is_refused_not_repaired() {
    let path = || "logo.png".to_string();
    let calls = [
        Call::File { path: path() },
        Call::Manifest { path: path() },
        Call::Matched { path: path(), pattern: "USER".into() },
        Call::ManifestField { path: path(), keys: "a".into() },
    ];
    for call in calls {
        let why = format!("{} \"logo.png\": not UTF-8 text (invalid byte at offset 5)", call.function());
        assert_eq!(read(&Binary, call).read, Err(why));
    }
}

#[test]
fn the_readers_of_a_changed_file_are_the_facts_that_name_it() {
    let page = [
        "A `1` <!-- fact: manifest(\"./p.json\").a -->",
        "",
        "B `2` <!-- fact: manifest(\"p.json\").b -->",
        "",
        "C `3` <!-- fact: manifest(\"q.json\").c -->",
        "",
        "D `{` <!-- fact: file(\"p.json\")[0] -->",
        "",
        "E `1` <!-- fact: manifestField(\"p.json\", \"a\") -->",
        "",
        "F `2` <!-- fact: matched(\"./p.json\", \"b.:(\\\\d)\") -->",
        "",
    ]
    .join("\n");
    let extra = [("p.json", r#"{"a":1,"b":2}"#), ("q.json", r#"{"c":3}"#)];
    let (files, attestations) = project(&page, &extra);
    let fact_files = load_fact_files(&files, &attestations);
    let readers = fact_files.readers(&attestations, &["p.json".to_string()]);
    let lines: Vec<(Reader, &str, usize)> =
        readers.iter().map(|r| (r.reader, r.file.as_str(), r.fact.line)).collect();
    assert_eq!(
        lines,
        [
            (Reader::Manifest, "p.json", 1),
            (Reader::Manifest, "p.json", 3),
            (Reader::File, "p.json", 7),
            (Reader::Manifest, "p.json", 9),
            (Reader::File, "p.json", 11),
        ]
    );
}

#[test]
fn matched_and_field_carry_their_values_by_path_then_argument() {
    let page = [
        "User `10001` <!-- fact: matched(\"Dockerfile\", r\"USER (\\d+)\") -->",
        "",
        "Image `node:22` <!-- fact: matched(\"Dockerfile\", `FROM (\\S+)`) -->",
        "",
        "Version `0.2.2` <!-- fact: manifestField(\".gitlab-ci.yml\", \"docattest.variables.VERSION\") -->",
        "",
        "Replicas `3` <!-- fact: manifestField(\".gitlab-ci.yml\", 'deploy.\"replica count\"') -->",
        "",
        "Tags <!-- fact: manifestField(\".gitlab-ci.yml\", \"deploy.tags\") -->",
        "- `a`",
        "",
    ]
    .join("\n");
    let ci = "docattest:\n  variables:\n    VERSION: 0.2.2\ndeploy:\n  replica count: 3\n  tags: [a]\n";
    let extra = [("Dockerfile", "FROM node:22\nUSER 10001\n"), (".gitlab-ci.yml", ci)];
    let (files, attestations) = project(&page, &extra);
    let fact_files = load_fact_files(&files, &attestations);
    assert_eq!(
        Value::Object(fact_files.session_params()),
        json!({
            "manifests": {},
            "matches": { "Dockerfile": { "USER (\\d+)": "10001", "FROM (\\S+)": "node:22" } },
            "manifestFields": { ".gitlab-ci.yml": {
                "docattest.variables.VERSION": "0.2.2", "deploy.\"replica count\"": 3, "deploy.tags": ["a"]
            } },
        })
    );
}

#[test]
fn a_call_that_gives_nothing_makes_its_fact_an_error_that_says_why() {
    let page = [
        "A `1` <!-- fact: matched(\"Dockerfile\", \"USER (\\\\d+):\") -->",
        "",
        "B `1` <!-- fact: matched(\"Dockerfile\", \"USER (\") -->",
        "",
        "C `1` <!-- fact: manifestField(\"ci.yml\", \"deploy.version\") -->",
        "",
        "D `1` <!-- fact: manifestField(\"ci.yml\", \"deploy..x\") -->",
        "",
        "E `1` <!-- fact: matched(\"gone.txt\", \"x\") + manifestField(\"../up.json\", \"x\") -->",
        "",
        "F `1` <!-- fact: manifestField(\"Dockerfile\", \"x\") -->",
        "",
    ]
    .join("\n");
    let extra = [("Dockerfile", "USER 10001\n"), ("ci.yml", "deploy:\n  image: app\n  replicas: 3\n")];
    let (files, mut attestations) = project(&page, &extra);
    load_fact_files(&files, &attestations).refuse_unreadable(&mut attestations);
    let errors: Vec<&str> = attestations.iter().filter_map(Attestation::problem).collect();
    assert_eq!(errors[0], r#"matched "Dockerfile": "USER (\d+):" matches nothing"#);
    assert!(
        errors[1].starts_with(r#"matched "Dockerfile": "USER (" is not a valid regex: "#),
        "{}",
        errors[1]
    );
    assert_eq!(
        errors[2..],
        [
            r#"manifestField "ci.yml" has no deploy.version: deploy has keys image, replicas"#,
            r#"manifestField "ci.yml": deploy..x has an empty key"#,
            r#"matched "gone.txt": no such file"#,
            r#"manifestField "Dockerfile": not a format docattest reads: JSON (.json), TOML (.toml), YAML (.yaml, .yml) or go.mod"#,
        ]
    );
}
