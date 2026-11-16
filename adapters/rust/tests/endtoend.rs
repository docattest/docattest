//! The Rust package as it is used: its two commands, built, with the protocol's
//! conformance suite and the docattest core (which the `docattest` command is built
//! with, and which carries the suite) on the sample project in `tests/fixture`.

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
        ran.status.success() && said.contains("conformance (rust): 13 of 13 checks hold"),
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

/// Runs the `docattest` command in a project, as a Rust user runs it. Nothing says where
/// the adapter is: the command finds it beside itself.
fn docattest(root: &Path, args: &[&str]) -> (String, i32) {
    let ran = Command::new(DOCATTEST)
        .args(args)
        .arg("--root")
        .arg(root)
        .env_remove("DOCATTEST_CORE")
        .env_remove("DOCATTEST_ADAPTER_RUST")
        .env_remove("DOCATTEST_NETWORK")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    let code = ran.status.code().unwrap_or(-1);
    assert_ne!(code, 2, "docattest {args:?}: an internal error\n{}", String::from_utf8_lossy(&ran.stderr));
    (String::from_utf8_lossy(&ran.stdout).into_owned(), code)
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
    let (out, code) = docattest(&fixture(), &["check", "--json"]);
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
    assert_eq!(docattest(&root, &["uncovered"]).0.trim(), "ERROR_CODES");
    assert!(docattest(&root, &["where", "quote"]).0.contains("docs/shipping.md:10 example"));
    let audit = docattest(&root, &["audit"]).0;
    for want in ["L5 list matches CARRIERS", "L11 column drifts from ERROR_CODES (missing: no_carrier)"] {
        assert!(audit.contains(want), "no {want:?} in {audit}");
    }
    assert!(docattest(&root, &["inventory"]).0.contains("@shipping CARRIERS  →  [post, courier, drone]"));
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

#[test]
fn impact_names_the_prose_a_change_touches() {
    let root = std::env::temp_dir().join(format!("docattest-rust-impact-{}", std::process::id()));
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
    if !(git(&["init", "-q", "-b", "main"]) && git(&["add", "-A"]) && git(&["commit", "-q", "-m", "base"])) {
        return eprintln!("impact was not tried: git could not make a repository");
    }
    let shipping = root.join("src/shipping.rs");
    let text = std::fs::read_to_string(&shipping).unwrap();
    std::fs::write(&shipping, text.replacen("(\"DE\", 2)", "(\"DE\", 3)", 1)).unwrap();
    let said = docattest(&root, &["impact"]).0;
    for want in ["affect 1 exported symbol", "quote", "docs/shipping.md:10 proven example"] {
        assert!(said.contains(want), "no {want:?} in {said}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
