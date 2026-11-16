//! Talking to the adapter: one process per conversation, started as the core starts it. Every
//! line is written at once and then the input ends, as a pipe allows; what comes back is read
//! to the end and held to the schemas.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;

use super::Adapter;
use super::expect::{Checked, ensure, same};
use crate::adapter::client::is_blank_line;
use crate::files::text_file::decode;
use crate::schema::Schemas;

/// How long a conversation may take before the adapter is killed.
const LIMIT: Duration = Duration::from_secs(120);

/// A line the suite sends: a message, or text written as it is.
pub enum Line {
    Message(Value),
    Text(&'static str),
}

impl From<Value> for Line {
    fn from(message: Value) -> Line {
        Line::Message(message)
    }
}

impl Line {
    fn text(&self) -> String {
        match self {
            Line::Message(message) => message.to_string(),
            Line::Text(text) => (*text).to_string(),
        }
    }
}

/// A finished conversation, as the checks look at it.
pub struct Conversation {
    /// The adapter's exit code (`None` when a signal ended it).
    pub code: Option<i32>,
    /// The lines on stdout that are not blank.
    pub out: Vec<String>,
    /// Those of them that are JSON, parsed.
    pub responses: Vec<Value>,
    pub stderr: String,
    /// What does not fit the schemas, of every line both ways.
    pub drift: Vec<String>,
}

impl Conversation {
    /// The one answer to request `id`.
    pub fn of(&self, id: u64) -> Checked<&Value> {
        Ok(&self.responses[self.position(id)?])
    }

    /// The result of request `id`, which must have one.
    pub fn result(&self, id: u64) -> Checked<&Value> {
        let answer = self.of(id)?;
        answer
            .get("result")
            .ok_or_else(|| format!("request {id} failed: {}\n{}", answer["error"], self.stderr))
    }

    /// Request `id` failed with an error of `kind`.
    pub fn failed_as(&self, id: u64, kind: &str) -> Checked {
        let answer = self.of(id)?;
        let Some(error) = answer.get("error") else {
            return Err(format!("request {id} should have failed, and answered {}", answer["result"]));
        };
        ensure(error["kind"] == kind, format!("request {id}: expected a {kind} error, got {error}"))
    }

    /// The result of request `id` is `expected`.
    pub fn answered(&self, id: u64, expected: &Value, what: &str) -> Checked {
        self.result(id).and_then(|result| same(result, expected, what))
    }

    /// The adapter exited 0 when its input ended.
    pub fn exited_cleanly(&self) -> Checked {
        ensure(self.code == Some(0), "the adapter exits 0 at end of input")
    }

    /// The answer to request `first` came before the answer to `then`.
    pub fn answered_before(&self, first: u64, then: u64, what: &str) -> Checked {
        ensure(self.position(first)? < self.position(then)?, what)
    }

    /// Where the one answer to request `id` came among the responses.
    fn position(&self, id: u64) -> Checked<usize> {
        let id_value = Value::from(id);
        let found: Vec<usize> =
            (0..self.responses.len()).filter(|&k| self.responses[k].get("id") == Some(&id_value)).collect();
        match found[..] {
            [at] => Ok(at),
            _ => Err(format!("request {id} has {} answers\n{}", found.len(), self.stderr)),
        }
    }
}

/// One conversation with `adapter`: `lines` in, everything out. Every line, both ways, is
/// held to the schemas, whatever a check looks at.
pub fn talk(adapter: &Adapter, lines: Vec<Line>, schemas: &Schemas) -> Checked<Conversation> {
    let sent: Vec<String> = lines.iter().map(Line::text).collect();
    let (code, stdout, stderr) =
        run(adapter, &sent).map_err(|e| format!("cannot run {} {:?}: {e}", adapter.program, adapter.args))?;
    let out: Vec<String> =
        stdout.split('\n').filter(|line| !is_blank_line(line.as_bytes())).map(String::from).collect();
    let responses: Vec<Value> = out.iter().filter_map(|line| serde_json::from_str(line).ok()).collect();
    let drift = schemas.check_exchange(&sent, &out);
    Ok(Conversation { code, out, responses, stderr, drift })
}

/// Runs `adapter` with `sent` as its input, each line ended by `\n`: its exit code, stdout
/// and stderr. Past [`LIMIT`] the adapter is killed, and what it wrote by then is kept.
fn run(adapter: &Adapter, sent: &[String]) -> std::io::Result<(Option<i32>, String, String)> {
    let mut child = crate::process::spawn(
        Command::new(adapter.program)
            .args(adapter.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?;
    let input: String = sent.iter().map(|line| format!("{line}\n")).collect();
    let mut stdin = child.stdin.take().expect("stdin is piped");
    // An adapter that exits without reading its input is answered by its exit, not a write error.
    let writer = thread::spawn(move || drop(stdin.write_all(input.as_bytes())));
    let stdout = read_in_background(child.stdout.take().expect("stdout is piped"));
    let stderr = read_in_background(child.stderr.take().expect("stderr is piped"));
    let out = within_limit(&stdout, &mut child)?;
    let status = child.wait()?;
    let _ = writer.join();
    let err = stderr.recv().map_err(std::io::Error::other)??;
    Ok((status.code(), out, err))
}

/// What `stdout` gives, waiting at most [`LIMIT`]; past it, the adapter is killed first.
fn within_limit(
    stdout: &mpsc::Receiver<std::io::Result<String>>,
    child: &mut Child,
) -> std::io::Result<String> {
    if let Ok(read) = stdout.recv_timeout(LIMIT) {
        return read;
    }
    child.kill()?;
    stdout.recv().map_err(std::io::Error::other)?
}

/// Everything `source` gives until it ends, as text, on the channel returned.
fn read_in_background(mut source: impl Read + Send + 'static) -> mpsc::Receiver<std::io::Result<String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = source.read_to_end(&mut bytes).map(|_| decode(bytes));
        // The receiver is gone only when the conversation has already failed.
        let _ = sender.send(read);
    });
    receiver
}
