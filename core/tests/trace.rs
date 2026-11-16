//! `DOCATTEST_TRACE` writes to stderr, and a trace nobody reads must change nothing: the
//! core used to panic on the first line it could not write, and to hang when that line was
//! the adapter reader's.

use std::process::{Command, Stdio};

#[test]
fn a_trace_nobody_reads_changes_nothing() {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    // Its reader is gone before the core starts: every write to stderr fails.
    drop(reader);
    let mut command = Command::new(env!("CARGO_BIN_EXE_docattest"));
    command
        .arg("--help")
        .env("DOCATTEST_TRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(writer);
    let child = docattest::process::spawn(&mut command).expect("the core starts");
    let output = child.wait_with_output().expect("the core exits");
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("docattest"));
}
