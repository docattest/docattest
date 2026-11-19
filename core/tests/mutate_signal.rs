//! A `mutate` stopped by a signal removes the copies it made beside the project before it
//! exits, as the signal would have ended it. The fake adapter (`fixtures/mutate/adapter.sh`)
//! takes long over every evaluation in a copy, so the run is stopped while a copy exists.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The page the fake adapter answers for: a fact on each of lines 1, 3, 5 and 7.
const PAGE: &str = "N is `3`. <!-- fact: N -->\n\nM is `7`. <!-- fact: M -->\n\nF gives `2`. <!-- fact: F() -->\n\nJ is `5`. <!-- fact: J -->\n";

/// A new project in the system's temporary directory, alone in a directory of its own so
/// that what is beside it is only what `mutate` made.
fn project() -> PathBuf {
    let parent = std::env::temp_dir().join(format!("docattest-mutate-signal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&parent);
    let root = parent.join("shop");
    let config = r#"{"pages":["docs/*.md"],"surfaces":{"default":"x"},"adapter":"fake"}"#;
    for (file, text) in
        [("docattest.config.json", config), ("docs/a.md", PAGE), ("src/n.txt", "3\n"), ("src/k.txt", "1\n")]
    {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    root
}

fn beside(root: &Path) -> Vec<String> {
    fs::read_dir(root.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "shop")
        .collect()
}

#[test]
fn a_run_stopped_by_sigterm_removes_its_copies() {
    let root = project();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mutate/adapter.sh");
    let mut command = Command::new(env!("CARGO_BIN_EXE_docattest"));
    command
        .args(["mutate", "--root"])
        .arg(&root)
        .env("DOCATTEST_ADAPTER_FAKE", format!("[\"sh\",\"{script}\"]"))
        .env("FAKE_ADAPTER_SLOW_IN_COPIES", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = docattest::process::spawn(&mut command).expect("the core starts");
    let started = Instant::now();
    while !beside(&root).iter().any(|name| name.contains(".docattest-mutate-")) {
        assert!(started.elapsed() < Duration::from_secs(20), "no copy was made");
        std::thread::sleep(Duration::from_millis(50));
    }
    // The copy's adapter is now evaluating, which takes long.
    std::thread::sleep(Duration::from_millis(500));
    let mut kill = Command::new("kill");
    kill.args(["-TERM", &child.id().to_string()]);
    assert!(docattest::process::spawn(&mut kill).unwrap().wait().unwrap().success());
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(128 + 15));
    assert_eq!(beside(&root), Vec::<String>::new(), "the copies and their locks are removed");
    let _ = fs::remove_dir_all(root.parent().unwrap());
}
