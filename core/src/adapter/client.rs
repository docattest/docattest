//! The core's side of PROTOCOL.md: one adapter child process per invocation, JSON lines on
//! its stdin and stdout, several requests in flight, answers in any order.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use serde_json::{Map, Value, json};

use crate::api::errors::{Error, Result};

type Reply = Result<Value>;

#[derive(Default)]
struct Shared {
    pending: HashMap<u64, Sender<Reply>>,
    /// Set once the adapter has gone: every later request fails with it.
    dead: Option<Error>,
}

/// A running adapter.
pub struct Client {
    child: Child,
    stdin: Mutex<Option<ChildStdin>>,
    shared: Arc<Mutex<Shared>>,
    next: AtomicU64,
    reader: Option<JoinHandle<()>>,
    name: String,
}

/// An answer that has not arrived yet.
pub struct Pending {
    rx: Receiver<Reply>,
}

impl Pending {
    /// Wait for the answer.
    pub fn wait(self) -> Result<Value> {
        self.rx
            .recv()
            .unwrap_or_else(|_| Err(Error::internal("the adapter connection closed without an answer")))
    }

    pub(crate) fn ready(reply: Reply) -> Pending {
        let (tx, rx) = channel();
        // The receiver is alive: it is returned with this sender's value queued.
        let _ = tx.send(reply);
        Pending { rx }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Client {
    /// Start the adapter from a command line (program and arguments), in `dir` when one is
    /// given. Its stderr is passed through for a human to read.
    pub fn spawn(argv: &[String], dir: Option<&Path>, name: &str) -> Result<Client> {
        let (program, args) =
            argv.split_first().ok_or_else(|| Error::user(format!("the {name} adapter command is empty")))?;
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        let mut child = crate::process::spawn(&mut command)
            .map_err(|e| Error::user(format!("cannot start the {name} adapter ({}): {e}", argv.join(" "))))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let reader = stdout.map(|out| {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || read_answers(BufReader::new(out), &shared))
        });
        Ok(Client {
            child,
            stdin: Mutex::new(stdin),
            shared,
            next: AtomicU64::new(1),
            reader,
            name: name.to_string(),
        })
    }

    /// Send a request; the answer comes later.
    pub fn send(&self, method: &str, params: Value) -> Pending {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        crate::trace::mark(&format!("send {id} {method}"));
        let (tx, rx) = channel();
        {
            let mut shared = lock(&self.shared);
            if let Some(dead) = &shared.dead {
                return Pending::ready(Err(dead.clone()));
            }
            shared.pending.insert(id, tx);
        }
        let mut line = serde_json::to_string(&json!({ "id": id, "method": method, "params": params }))
            .unwrap_or_default();
        line.push('\n');
        let written = match lock(&self.stdin).as_mut() {
            Some(stdin) => stdin.write_all(line.as_bytes()).and_then(|()| stdin.flush()),
            None => Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "input closed")),
        };
        if written.is_err() {
            // The reader reports why the adapter went away; until then, wait for it.
            let mut shared = lock(&self.shared);
            if let Some(dead) = &shared.dead {
                let dead = dead.clone();
                shared.pending.remove(&id);
                return Pending::ready(Err(dead));
            }
        }
        Pending { rx }
    }

    /// End the conversation: close the adapter's input and wait for it to exit.
    pub fn finish(mut self) -> Result<()> {
        self.end()
    }

    fn end(&mut self) -> Result<()> {
        lock(&self.stdin).take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let status = self
            .child
            .wait()
            .map_err(|e| Error::internal(format!("waiting for the {} adapter: {e}", self.name)))?;
        if !status.success() {
            return Err(Error::internal(format!("the {} adapter exited with {status}", self.name)));
        }
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // An abandoned conversation (an error elsewhere): close input, let it exit.
        if self.reader.is_some() {
            let _ = self.end();
        }
    }
}

/// Tells everyone still waiting that no answer will come, when the reader ends: at the end of
/// the adapter's output, on a bad line, or by a panic, which must fail the requests in
/// flight like any other end and never leave them waiting.
struct HangUp<'a> {
    shared: &'a Mutex<Shared>,
    failure: Option<Error>,
}

impl Drop for HangUp<'_> {
    fn drop(&mut self) {
        let mut shared = lock(self.shared);
        let error = self.failure.take().unwrap_or_else(|| {
            Error::internal(if std::thread::panicking() {
                "the adapter's answers could not be read"
            } else {
                "the adapter exited before answering"
            })
        });
        for (_, tx) in shared.pending.drain() {
            let _ = tx.send(Err(error.clone()));
        }
        shared.dead = Some(error);
    }
}

/// A line that carries no message: nothing but whitespace (PROTOCOL.md §1, a blank line is
/// ignored). The conformance suite skips the same lines.
pub(crate) fn is_blank_line(line: &[u8]) -> bool {
    line.iter().all(u8::is_ascii_whitespace)
}

fn read_answers(reader: impl BufRead, shared: &Mutex<Shared>) {
    let mut end = HangUp { shared, failure: None };
    read_until_failure(reader, shared, &mut end.failure);
}

fn read_until_failure(reader: impl BufRead, shared: &Mutex<Shared>, failure: &mut Option<Error>) {
    for line in reader.split(b'\n') {
        let Ok(bytes) = line else {
            *failure = Some(Error::internal("cannot read the adapter's output"));
            break;
        };
        if is_blank_line(&bytes) {
            continue;
        }
        match answer(&bytes) {
            Ok((id, reply)) => {
                crate::trace::mark(&format!("answer {id}"));
                let tx = lock(shared).pending.remove(&id);
                match tx {
                    Some(tx) => {
                        let _ = tx.send(reply);
                    }
                    None => {
                        *failure = Some(Error::internal(format!(
                            "the adapter answered request {id}, which is not pending"
                        )));
                        break;
                    }
                }
            }
            Err(e) => {
                *failure = Some(e);
                break;
            }
        }
    }
}

/// One response line: its id and the result, or the error it carries.
fn answer(bytes: &[u8]) -> Result<(u64, Reply)> {
    let preview = || {
        let text = String::from_utf8_lossy(bytes);
        let short: String = text.chars().take(80).collect();
        if short.len() < text.len() { format!("{short}…") } else { short }
    };
    let not_a_response =
        || Error::internal(format!("the adapter wrote a line that is not a response: {}", preview()));
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => serde_json::from_str(&without_lone_surrogates(&String::from_utf8_lossy(bytes)))
            .map_err(|_| not_a_response())?,
    };
    let Value::Object(mut object) = value else {
        return Err(not_a_response());
    };
    let id = object.get("id").and_then(Value::as_u64).ok_or_else(not_a_response)?;
    if let Some(result) = object.remove("result") {
        return Ok((id, Ok(result)));
    }
    let Some(Value::Object(error)) = object.remove("error") else {
        return Err(not_a_response());
    };
    Ok((id, Err(wire_error(&error))))
}

fn wire_error(error: &Map<String, Value>) -> Error {
    let message = error.get("message").and_then(Value::as_str).unwrap_or("").to_string();
    match error.get("kind").and_then(Value::as_str) {
        Some("user") => Error::user_verbatim(message),
        _ => Error::internal(message),
    }
}

/// An adapter may send a lone surrogate as a `\udXXX` escape (a JavaScript adapter's
/// `JSON.stringify` does, for a string that holds one), which serde_json refuses. A Rust
/// string cannot hold one either, so each becomes U+FFFD; pairs are kept.
fn without_lone_surrogates(text: &str) -> String {
    let hex = |s: &str| u16::from_str_radix(s, 16).ok();
    let unit = |t: &str, at: usize| -> Option<u16> {
        let b = t.as_bytes();
        (b.get(at) == Some(&b'\\') && b.get(at + 1) == Some(&b'u')).then_some(())?;
        t.get(at + 2..at + 6).and_then(hex)
    };
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with("\\\\") {
            out.push_str("\\\\");
            at += 2;
            continue;
        }
        if let Some(u) = unit(text, at) {
            let high = (0xD800..0xDC00).contains(&u);
            let low = (0xDC00..0xE000).contains(&u);
            if high && unit(text, at + 6).is_some_and(|n| (0xDC00..0xE000).contains(&n)) {
                out.push_str(&text[at..at + 12]);
                at += 12;
                continue;
            }
            if high || low {
                out.push_str("\\ufffd");
                at += 6;
                continue;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push(c);
        at += c.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_surrogates_become_replacement_characters() {
        let line = br#"{"id":1,"result":"a\ud800b\ud83d\ude00\\ud800"}"#;
        let (id, reply) = answer(line).unwrap();
        assert_eq!(id, 1);
        assert_eq!(reply.unwrap(), serde_json::json!("a\u{FFFD}b\u{1F600}\\ud800"));
    }

    /// Output that cannot be read at all: reading it panics.
    struct Broken;

    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("the reader broke");
        }
    }

    #[test]
    fn a_reader_that_panics_fails_what_is_pending() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = channel();
        lock(&shared).pending.insert(1, tx);
        let theirs = Arc::clone(&shared);
        let reader = std::thread::spawn(move || read_answers(BufReader::new(Broken), &theirs));
        assert!(reader.join().is_err());
        // Without an answer this would wait for ever: the sender lives on in `pending`.
        let reply = Pending { rx }.wait();
        assert_eq!(reply, Err(Error::internal("the adapter's answers could not be read")));
        assert!(lock(&shared).dead.is_some());
    }

    #[test]
    fn errors_map_to_their_kind() {
        let (_, reply) = answer(br#"{"id":2,"error":{"kind":"user","message":"docattest: x"}}"#).unwrap();
        assert_eq!(reply, Err(Error::User("docattest: x".into())));
        assert!(answer(b"not json").is_err());
    }
}
