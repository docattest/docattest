//! The tests citations name, run by `cargo test` as `runTests` asks: each test writes a
//! till's crate to the temporary directory and runs some of its tests with this machine's
//! cargo.

mod project;

use project::{Written, open_session};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

const MANIFEST: &str = "[package]\nname = \"till\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";

/// A library whose tests pass, fail, are ignored, or are left out of the build.
const LIB: &str = r#"
pub mod money;

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn adds() {
        assert_eq!(super::add(1, 1), 2);
    }

    #[test]
    fn subtracts() {
        println!("working it out");
        assert_eq!(super::add(2, -1), 0, "two minus one is not zero");
    }

    #[test]
    #[ignore = "needs a printer"]
    fn prints() {}

    #[test]
    #[ignore]
    fn waits() {}

    #[test]
    #[should_panic(expected = "no change")]
    fn refuses() {
        panic!("no change");
    }

    #[test]
    fn adds_more() {
        panic!("ran with adds");
    }

    #[cfg(any())]
    #[test]
    fn left_out() {}

    #[test]
    fn sleeps() {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
"#;

const MONEY: &str = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn rounds() {}\n}\n";

const MATH: &str = "#[test]\nfn multiplies() {\n    assert_eq!(2 * 2, 4);\n}\n";

const BROKEN: &str = "#[test]\nfn breaks() {\n    let n: u32 = \"no\";\n}\n";

const ORPHAN: &str = "#[test]\nfn alone() {}\n";

/// The till's crate on disk, with a session open on it.
fn till(test: &str) -> Written {
    let files = [
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", LIB),
        ("src/money.rs", MONEY),
        ("src/orphan.rs", ORPHAN),
        ("tests/math.rs", MATH),
        ("tests/broken.rs", BROKEN),
    ];
    Written::open(test, &files, json!([{ "name": "default", "entry": "till" }]))
}

/// A cited test as `runTests` names it: its file, then its path.
fn cited(file: &str, path: &[&str]) -> Value {
    let (title, describes) = path.split_last().unwrap();
    json!({ "file": file, "describes": describes, "title": title })
}

fn run_tests(written: &Written, tests: &[Value], timeout: Duration) -> Vec<Value> {
    let ran = written.call("runTests", json!({ "tests": tests, "timeoutMs": timeout.as_millis() as u64 }));
    ran.as_array().unwrap().clone()
}

/// A run's status, and its message, when it has one.
fn outcome(run: &Value) -> (&str, &str) {
    (run["status"].as_str().unwrap(), run["message"].as_str().unwrap_or(""))
}

const TEN_MINUTES: Duration = Duration::from_secs(600);

#[test]
fn each_cited_test_is_run_exactly_and_reported_as_libtest_reports_it() {
    let written = till("cited-each");
    let tests = [
        cited("src/lib.rs", &["tests", "adds"]),
        cited("src/lib.rs", &["tests", "subtracts"]),
        cited("src/lib.rs", &["tests", "prints"]),
        cited("src/lib.rs", &["tests", "waits"]),
        cited("src/lib.rs", &["tests", "refuses"]),
        cited("src/lib.rs", &["tests", "left_out"]),
        cited("src/money.rs", &["tests", "rounds"]),
        cited("tests/math.rs", &["multiplies"]),
        cited("tests/broken.rs", &["breaks"]),
        cited("src/orphan.rs", &["alone"]),
    ];
    let ran = run_tests(&written, &tests, TEN_MINUTES);
    let outcomes: Vec<(&str, &str)> = ran.iter().map(outcome).collect();
    assert_eq!(outcomes[0], ("passed", ""));
    let (status, failure) = outcomes[1];
    assert_eq!(status, "failed");
    assert!(failure.contains("two minus one is not zero"), "{failure}");
    assert!(!failure.contains("working it out"), "{failure}");
    assert_eq!(outcomes[2], ("skipped", "needs a printer"));
    assert_eq!(outcomes[3], ("skipped", "cargo test ignored it: it is marked #[ignore]"));
    assert_eq!(outcomes[4], ("passed", ""));
    assert_eq!(outcomes[5], ("notRun", "cargo test --package till --lib lists no test tests > left_out"));
    assert_eq!(outcomes[6], ("passed", ""));
    assert_eq!(outcomes[7], ("passed", ""));
    let (status, error) = outcomes[8];
    assert_eq!(status, "failed");
    assert!(error.starts_with("error[E0308]: mismatched types"), "{error}");
    assert_eq!(outcomes[9], ("notRun", "src/orphan.rs is not a module of any target cargo test builds"));
}

#[test]
fn a_run_that_overruns_is_ended_and_fails_its_tests() {
    let written = till("cited-overrun");
    // The first run builds the library, which is not what the limit is for.
    let built = run_tests(&written, &[cited("src/lib.rs", &["tests", "adds"])], TEN_MINUTES);
    assert_eq!(outcome(&built[0]), ("passed", ""));
    let started = Instant::now();
    let ran = run_tests(&written, &[cited("src/lib.rs", &["tests", "sleeps"])], Duration::from_secs(5));
    assert_eq!(outcome(&ran[0]), ("failed", "cargo test did not finish within 5 s"));
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn nothing_runs_in_a_project_in_memory() {
    let files = json!({ "Cargo.toml": MANIFEST, "src/lib.rs": LIB, "src/money.rs": MONEY });
    let surfaces = json!([{ "name": "default", "entry": "till" }]);
    let session = open_session(Path::new("/till-in-memory"), surfaces, json!({ "files": files }));
    let tests = json!([cited("src/lib.rs", &["tests", "adds"]), cited("src/money.rs", &["tests", "rounds"])]);
    let ran = project::call(&session, "runTests", json!({ "tests": tests, "timeoutMs": 600000 }));
    session.close();
    let in_memory = json!({
        "status": "notRun",
        "message": "the project is in memory (open.files), where nothing runs: cargo test runs a project on disk"
    });
    assert_eq!(ran, json!([in_memory, in_memory]));
}
