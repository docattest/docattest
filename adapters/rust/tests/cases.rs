//! How the rows of cases tables run: what they share, and what ends a process. Each test
//! writes a project whose setup file's runners say where they ran.

mod project;

use docattest_rust::protocol::Session;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

const MANIFEST: &str = "[package]\nname = \"rows\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";

/// Runners that report where they ran: `count` fails each row with its number in its
/// process and the process's id, after doing what the row's `then` says; `meet` waits for
/// the files the others write.
const SETUP: &str = r#"
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

type Row = HashMap<String, String>;

pub const CASES: &[(&str, fn(&Row) -> Result<(), String>)] = &[("count", count), ("meet", meet)];

static ROWS: AtomicUsize = AtomicUsize::new(0);
static LEFT_BEHIND: AtomicUsize = AtomicUsize::new(0);

fn count(row: &Row) -> Result<(), String> {
    let seen = ROWS.fetch_add(1, Ordering::SeqCst) + 1;
    match row["then"].as_str() {
        "panic" => panic!("row {seen}"),
        "exit" => std::process::exit(4),
        "hang" => loop { std::thread::sleep(Duration::from_secs(1)) },
        "start a thread" => drop(std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            LEFT_BEHIND.store(1, Ordering::SeqCst);
        })),
        "see the thread" => {
            std::thread::sleep(Duration::from_millis(500));
            if LEFT_BEHIND.load(Ordering::SeqCst) == 0 {
                return Err("the thread is gone".into());
            }
        }
        _ => {}
    }
    Err(format!("row {seen} of process {}", std::process::id()))
}

pub fn meet(row: &Row) -> Result<(), String> {
    std::fs::write(&row["me"], "").map_err(|err| err.to_string())?;
    for _ in 0..200 {
        if row["others"].split(' ').all(|other| std::path::Path::new(other).exists()) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!("{} ran alone", row["me"]))
}
"#;

/// A project with these runners, and a session on it whose programs have this long.
fn open(test: &str, timeout_ms: u64, workers: u64) -> (PathBuf, Arc<dyn Session>) {
    let config =
        r#"{ "pages": [], "surfaces": { "default": "rows" }, "setup": "setup.rs", "adapter": "rust" }"#;
    let files = [
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", ""),
        ("setup.rs", SETUP),
        ("docattest.config.json", config),
    ];
    let root = project::write(&format!("cases-{test}"), &files);
    let told = json!({ "setup": "setup.rs", "timeoutMs": timeout_ms, "workers": workers });
    let session = project::open_session(&root, json!([{ "name": "default", "entry": "rows" }]), told);
    (root, session)
}

/// A table of a runner, with a row for each set of cells.
fn table(runner: &str, line: usize, rows: &[Value]) -> Value {
    let header: Vec<&str> = rows[0].as_object().unwrap().keys().map(String::as_str).collect();
    let rows: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(k, values)| json!({ "line": line + 2 + k, "name": format!("r{k}"), "values": values }))
        .collect();
    json!({ "id": format!("{runner}{line}"), "page": "docs/p.md", "line": line, "claim": "", "kind": "cases", "runner": runner, "header": header, "rows": rows })
}

/// A message without the page, line and name of its row.
fn said(result: &Value) -> String {
    let message = result["message"].as_str().unwrap();
    message.split_once(": ").map_or(message, |(_, said)| said).to_string()
}

#[test]
fn a_tables_rows_share_its_process() {
    let (root, session) = open("process", 2000, 2);
    let then = ["", "", "panic", "", "start a thread", "see the thread", "hang", "", "exit", ""];
    let rows: Vec<Value> = then.iter().map(|then| json!({ "then": then })).collect();
    let results =
        project::call(&session, "run", json!({ "examples": [], "cases": [table("count", 1, &rows)] }));
    session.close();
    let _ = std::fs::remove_dir_all(&root);
    let got: Vec<String> = results.as_array().unwrap().iter().map(said).collect();
    let process = |k: usize| got[k].rsplit(' ').next().unwrap().to_string();
    let wanted = [
        (0, "row 1 of process"),
        (1, "row 2 of process"),
        (2, "panicked: row 3"),
        (3, "row 4 of process"),
        (4, "row 5 of process"),
        (5, "row 6 of process"),
        (6, "timed out after 2000 ms"),
        (7, "row 1 of process"),
        (8, "exited with status 4"),
        (9, "row 1 of process"),
    ];
    for (k, want) in wanted {
        assert!(got[k].starts_with(want), "row {k}: {got:?}");
    }
    assert!([1, 3, 4, 5].iter().all(|&k| process(k) == process(0)), "{got:?}");
    assert!(process(7) != process(0) && process(9) != process(7), "{got:?}");
}

#[test]
fn tables_run_side_by_side() {
    let (root, session) = open("side-by-side", 30_000, 3);
    let meet = |me: &str, others: &str| json!({ "me": me, "others": others });
    let block = json!({ "id": "block", "page": "docs/p.md", "line": 30, "claim": "", "kind": "example", "lang": "rust", "mode": "check", "network": false, "weak": false,
        "code": "std::fs::write(\"block\", \"\").unwrap();\nwhile !(std::path::Path::new(\"a\").exists() && std::path::Path::new(\"b\").exists()) {\n    std::thread::sleep(std::time::Duration::from_millis(100));\n}" });
    let tables = [table("meet", 1, &[meet("a", "b block")]), table("meet", 10, &[meet("b", "a block")])];
    let results = project::call(&session, "run", json!({ "examples": [block], "cases": tables }));
    session.close();
    let _ = std::fs::remove_dir_all(&root);
    let statuses: Vec<&str> =
        results.as_array().unwrap().iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["pass", "pass", "pass"], "{results}");
}
