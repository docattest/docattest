//! Standing between a core and an adapter and writing down every line that passes. Each line
//! of the log is `{"to":"adapter"|"core","line":"…"}`, and the log is appended to, so one file
//! can hold several invocations. A conversation is appended whole, in one write, when the
//! adapter exits: a core that checks a page whose commands run docattest again talks to a
//! second recorder meanwhile, and the two conversations must not interleave. Nothing is
//! changed on the way through; [`super::logs`] holds the log to the schemas.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;

use serde_json::json;

use super::Adapter;
use crate::files::text_file::decode;

/// Which way a recorded line went.
#[derive(Clone, Copy)]
enum Way {
    ToAdapter,
    ToCore,
}

/// The conversation so far, shared by the two directions.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<String>>);

impl Log {
    fn note(&self, way: Way, line: &str) {
        let to = match way {
            Way::ToAdapter => "adapter",
            Way::ToCore => "core",
        };
        let entry = format!("{}\n", json!({ "to": to, "line": line }));
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push_str(&entry);
    }

    /// Append the conversation to the log at `path`, in one write.
    fn append_to(&self, path: &Path) -> std::io::Result<()> {
        let text = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        OpenOptions::new().create(true).append(true).open(path)?.write_all(text.as_bytes())
    }
}

/// Runs `adapter`, passing this process's stdin to it and its stdout to `out`,
/// line by line, and noting every line in the log at `path`. The adapter's stderr is its
/// own. Returns the adapter's exit code.
pub fn record(path: &Path, adapter: &Adapter, out: &mut dyn Write) -> std::io::Result<i32> {
    // Opened at the start too, so a log that cannot be written fails before the adapter runs.
    OpenOptions::new().create(true).append(true).open(path)?;
    let log = Log::default();
    let mut child = crate::process::spawn(
        Command::new(adapter.program)
            .args(adapter.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()),
    )?;
    let failure = pass_requests_in_background(child.stdin.take().expect("stdin is piped"), log.clone());
    pass_responses(BufReader::new(child.stdout.take().expect("stdout is piped")), &log, out)?;
    let code = child.wait()?.code().unwrap_or(1);
    log.append_to(path)?;
    match failure.try_recv() {
        Ok(error) => Err(error),
        Err(_) => Ok(code),
    }
}

/// [`pass_requests`] on a thread of its own, which is not joined: the core may keep its end
/// open after the adapter has exited. A failure to read a request is sent back instead.
fn pass_requests_in_background(to_adapter: ChildStdin, log: Log) -> mpsc::Receiver<std::io::Error> {
    let (failed, failure) = mpsc::channel();
    thread::spawn(move || {
        if let Err(error) = pass_requests(to_adapter, &log) {
            let _ = failed.send(error);
        }
    });
    failure
}

/// Every line the adapter writes, noted and passed to `out` as it comes.
fn pass_responses(mut from_adapter: impl BufRead, log: &Log, out: &mut dyn Write) -> std::io::Result<()> {
    while let Some(line) = read_line(&mut from_adapter)? {
        log.note(Way::ToCore, &line);
        writeln!(out, "{line}")?;
        out.flush()?;
    }
    Ok(())
}

/// Every line of this process's stdin, noted and passed to the adapter; at its end, the
/// adapter's input ends too. An adapter that stopped reading is answered by its exit.
fn pass_requests(mut to_adapter: ChildStdin, log: &Log) -> std::io::Result<()> {
    let mut requests = std::io::stdin().lock();
    while let Some(line) = read_line(&mut requests)? {
        log.note(Way::ToAdapter, &line);
        if to_adapter.write_all(format!("{line}\n").as_bytes()).is_err() {
            break;
        }
    }
    Ok(())
}

/// The next line, without its `\n`; none at the end. A protocol line ends at `\n` only, so
/// a `\r` before it is kept.
fn read_line(input: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::new();
    if input.read_until(b'\n', &mut bytes)? == 0 {
        return Ok(None);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(Some(decode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversations_that_overlap_are_appended_one_after_the_other() {
        let dir = crate::git::tests::Temp::new("record");
        let path = dir.0.join("log.jsonl");
        let (outer, inner) = (Log::default(), Log::default());
        outer.note(Way::ToAdapter, "1");
        inner.note(Way::ToAdapter, "a");
        outer.note(Way::ToCore, "2");
        inner.note(Way::ToCore, "b");
        inner.append_to(&path).unwrap();
        outer.append_to(&path).unwrap();
        let entry = |to: &str, line: &str| format!("{{\"to\":\"{to}\",\"line\":\"{line}\"}}\n");
        let expected = [entry("adapter", "a"), entry("core", "b"), entry("adapter", "1"), entry("core", "2")];
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected.concat());
    }

    #[test]
    fn a_line_ends_at_a_newline_only() {
        let mut input = "a\u{2028}b\r\nc".as_bytes();
        assert_eq!(read_line(&mut input).unwrap().as_deref(), Some("a\u{2028}b\r"));
        assert_eq!(read_line(&mut input).unwrap().as_deref(), Some("c"));
        assert_eq!(read_line(&mut input).unwrap(), None);
    }
}
