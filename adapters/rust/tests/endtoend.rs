//! The Rust package as it is used: its two commands, built, with the protocol's
//! conformance suite and the docattest core (which the `docattest` command is built
//! with, and which carries the suite) on the sample project in `tests/fixture`.

mod project;

use project::docattest;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const PACKAGE: &str = env!("CARGO_MANIFEST_DIR");
const DOCATTEST: &str = env!("CARGO_BIN_EXE_docattest");
const ADAPTER: &str = env!("CARGO_BIN_EXE_docattest-adapter-rust");

fn fixture() -> PathBuf {
    Path::new(PACKAGE).join("tests/fixture")
}

#[test]
fn the_adapter_conforms_to_the_protocol() {
    let kit = Path::new(PACKAGE).join("conformance.json");
    let ran =
        Command::new(DOCATTEST).args(["conformance", "run"]).arg(kit).args(["--", ADAPTER]).output().unwrap();
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(
        ran.status.success() && said.contains("conformance (rust): 14 of 14 checks hold"),
        "{said}{}",
        String::from_utf8_lossy(&ran.stderr)
    );
}

#[test]
fn a_trace_variable_set_to_nothing_asks_for_no_trace() {
    let traced = |value: &str| {
        let ran = Command::new(DOCATTEST).arg("--version").env("DOCATTEST_TRACE", value).output().unwrap();
        String::from_utf8_lossy(&ran.stderr).into_owned()
    };
    assert_eq!(traced(""), "");
    assert!(traced("1").starts_with("[trace launcher] core (built in)\n"), "{}", traced("1"));
}

/// What `check` says of each attestation of a page of the fixture: line, kind, status.
fn statuses(report: &Value, page: &str) -> Vec<(u64, String, String)> {
    let results = report["results"].as_array().unwrap().iter().filter(|result| result["page"] == page);
    results
        .map(|r| {
            (
                r["line"].as_u64().unwrap(),
                r["kind"].as_str().unwrap().to_string(),
                r["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn expected(lines: &[(u64, &str, &str)]) -> Vec<(u64, String, String)> {
    lines.iter().map(|(line, kind, status)| (*line, kind.to_string(), status.to_string())).collect()
}

#[test]
fn check_judges_every_attestation_of_the_fixture() {
    let (out, code) = docattest(&fixture(), &["check", "--json"], &[]);
    let report: Value = serde_json::from_str(&out).unwrap_or_else(|_| panic!("not JSON: {out}"));
    assert_eq!(code, 1);
    let broken = [
        (5, "example", "fail"),
        (11, "example", "fail"),
        (15, "example", "fail"),
        (19, "example", "fail"),
        (23, "example", "fail"),
        (27, "example", "error"),
        (31, "fact", "fail"),
        (33, "fact", "fail"),
        (37, "fact", "error"),
        (42, "cases", "fail"),
        (43, "cases", "fail"),
        (44, "cases", "fail"),
        (46, "cases", "error"),
        (51, "test", "error"),
        (53, "test", "error"),
    ];
    let pricing = [
        (5, "example", "pass"),
        (14, "example", "pass"),
        (25, "example", "pass"),
        (35, "example", "pass"),
        (43, "example", "pass"),
        (48, "fact", "pass"),
        (49, "fact", "pass"),
        (51, "fact", "pass"),
        (56, "fact", "pass"),
        (63, "fact", "pass"),
        (68, "fact", "pass"),
        (73, "fact", "pass"),
        (78, "cases", "pass"),
        (79, "cases", "pass"),
        (80, "cases", "pass"),
        (82, "test", "pass"),
    ];
    let shipping = [
        (3, "fact", "pass"),
        (10, "example", "pass"),
        (18, "example", "pass"),
        (24, "example", "skipped"),
        (33, "cases", "pass"),
        (34, "cases", "pass"),
    ];
    assert_eq!(statuses(&report, "docs/broken.md"), expected(&broken));
    assert_eq!(statuses(&report, "docs/pricing.md"), expected(&pricing));
    assert_eq!(statuses(&report, "docs/shipping.md"), expected(&shipping));
    assert_eq!(
        statuses(&report, "src/lib.rs"),
        expected(&[(46, "example", "pass"), (60, "example", "pass")])
    );
}

#[test]
fn the_index_and_the_audit_read_the_fixture() {
    let root = fixture();
    // `ERROR_CODES` is attested through the setup function a fact calls.
    assert_eq!(docattest(&root, &["uncovered"], &[]).0.trim(), "every public symbol is attested");
    assert!(docattest(&root, &["where", "quote"], &[]).0.contains("docs/shipping.md:10 example"));
    let audit = docattest(&root, &["audit"], &[]).0;
    for want in ["L5 list matches CARRIERS", "L11 column drifts from ERROR_CODES (missing: no_carrier)"] {
        assert!(audit.contains(want), "no {want:?} in {audit}");
    }
    assert!(
        docattest(&root, &["audit", "--inventory"], &[])
            .0
            .contains("@shipping CARRIERS  →  [post, courier, drone]")
    );
}

/// Copies a directory, without what cargo built in it.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten().filter(|entry| entry.file_name() != "target") {
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if source.is_dir() {
            copy(&source, &target);
        } else {
            std::fs::copy(&source, &target).unwrap();
        }
    }
}

/// A copy of the fixture committed to a git repository of its own, named for its test;
/// `None` when git cannot make one.
fn committed_fixture(test: &str) -> Option<PathBuf> {
    let root = std::env::temp_dir().join(format!("docattest-rust-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy(&fixture(), &root);
    let git = |args: &[&str]| {
        let identity = ["-c", "user.name=t", "-c", "user.email=t@example.invalid"];
        Command::new("git")
            .args(identity)
            .args(args)
            .current_dir(&root)
            .output()
            .is_ok_and(|ran| ran.status.success())
    };
    let made =
        git(&["init", "-q", "-b", "main"]) && git(&["add", "-A"]) && git(&["commit", "-q", "-m", "base"]);
    made.then_some(root)
}

/// Replaces the first occurrence of some text in a file of a project.
fn edit(root: &Path, file: &str, from: &str, to: &str) {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{file} does not hold {from:?}");
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn impact_names_the_prose_a_change_touches() {
    let Some(root) = committed_fixture("impact") else {
        return eprintln!("impact was not tried: git could not make a repository");
    };
    edit(&root, "src/shipping.rs", "(\"DE\", 2)", "(\"DE\", 3)");
    let said = docattest(&root, &["impact"], &[]).0;
    let _ = std::fs::remove_dir_all(&root);
    for want in ["affect 2 symbols", "DAYS", "quote", "docs/shipping.md:10 proven example"] {
        assert!(said.contains(want), "no {want:?} in {said}");
    }
}

#[test]
fn impact_of_a_constant_read_from_source() {
    let Some(root) = committed_fixture("impact-constants") else {
        return eprintln!("impact was not tried: git could not make a repository");
    };
    let page = "Germany is served in `2` days. <!-- fact @shipping: DAYS[1].1 -->\n\nThe port is `8080`. <!-- fact @shopd: LISTEN_PORT -->\n";
    std::fs::write(root.join("docs/constants.md"), page).unwrap();
    edit(&root, "src/shipping.rs", "(\"DE\", 2)", "(\"DE\", 3)");
    edit(&root, "src/bin/shopd.rs", "LISTEN_PORT: u16 = 8080", "LISTEN_PORT: u16 = 8081");
    let said = docattest(&root, &["impact", "--json"], &[]).0;
    let _ = std::fs::remove_dir_all(&root);
    let impact: Value = serde_json::from_str(&said).unwrap_or_else(|_| panic!("not JSON: {said}"));
    let references = impact["references"].as_array().unwrap().iter();
    let facts: Vec<String> = references
        .filter(|reference| reference["page"] == "docs/constants.md")
        .map(|reference| {
            format!("{} {}", reference["symbol"].as_str().unwrap(), reference["kind"].as_str().unwrap())
        })
        .collect();
    assert_eq!(facts, ["DAYS fact", "LISTEN_PORT fact"], "{said}");
}
