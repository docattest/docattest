//! The loop that serves requests: one JSON value per line in, one per line out.

use super::Hello;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Why a request has no result.
#[derive(Debug)]
pub enum Error {
    /// A mistake in the project; the core prints the message as it is.
    User(String),
    /// A bug in the adapter, or a mistake by the core.
    Internal(String),
}

impl Error {
    /// An internal error from anything that says what went wrong.
    pub fn internal(why: impl std::fmt::Display) -> Self {
        Error::Internal(why.to_string())
    }
}

/// The answer to one request.
pub type Answer = Result<Value, Error>;

/// Reads a method's params; params of the wrong shape are the core's mistake.
pub fn decode<T: DeserializeOwned>(params: Value) -> Result<T, Error> {
    serde_json::from_value(params).map_err(|err| Error::Internal(format!("params of the wrong shape: {err}")))
}

/// The one project `open` creates.
pub trait Session: Send + Sync {
    /// Answers a method of the session; `None` for a method it does not have.
    fn call(&self, method: &str, params: Value) -> Option<Answer>;
    /// Releases what the session holds.
    fn close(&self);
}

/// What a language gives the server to serve.
pub struct Adapter {
    /// The greeting, the same in every process.
    pub hello: Hello,
    /// Answers a question about text, which needs no session; `None` for any other method.
    pub language: fn(method: &str, params: Value) -> Option<Answer>,
    /// Creates the session.
    pub open: fn(params: Value) -> Result<Arc<dyn Session>, Error>,
}

/// Where a process is in the life of its one session.
#[derive(Clone)]
enum Life {
    NotOpen,
    Open(Arc<dyn Session>),
    Closed,
}

struct Server<W: Write + Send + 'static> {
    adapter: Adapter,
    out: Arc<Mutex<W>>,
    greeted: bool,
    life: Life,
    pending: Vec<JoinHandle<()>>,
}

/// Answers the requests on `input` until it ends; it then writes the answers still in
/// flight and closes the session.
pub fn serve<W: Write + Send + 'static>(input: impl BufRead, out: W, adapter: Adapter) {
    let out = Arc::new(Mutex::new(out));
    let mut server = Server { adapter, out, greeted: false, life: Life::NotOpen, pending: Vec::new() };
    for line in input.lines() {
        let Ok(line) = line else { break };
        if !line.trim().is_empty() {
            server.receive(&line);
        }
    }
    server.settle();
    server.close_session();
}

impl<W: Write + Send + 'static> Server<W> {
    /// Reads one line as a request and routes it; a line that is no request, or has no
    /// usable id, is answered with a null id.
    fn receive(&mut self, line: &str) {
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            return write(&self.out, &Value::Null, Err(Error::internal("a line that is not a request")));
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if !id.as_u64().is_some_and(|n| n > 0) {
            return write(
                &self.out,
                &Value::Null,
                Err(Error::internal("a request without a positive integer id")),
            );
        }
        let method = request.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        self.route(id, &method, params);
    }

    fn route(&mut self, id: Value, method: &str, params: Value) {
        let answer = match method {
            "hello" => self.hello(&params),
            _ if !self.greeted => {
                Err(Error::Internal(format!("\"{method}\" before hello: hello comes first")))
            }
            "open" => self.open(params),
            "close" => self.close(),
            _ => return self.later(id, method.to_string(), params),
        };
        write(&self.out, &id, answer);
    }

    /// Answers a request on its own thread, so that a slow one holds up no other.
    fn later(&mut self, id: Value, method: String, params: Value) {
        let (language, life) = (self.adapter.language, self.life.clone());
        let out = Arc::clone(&self.out);
        self.pending.push(std::thread::spawn(move || {
            let answer = catch_unwind(AssertUnwindSafe(|| {
                language(&method, params.clone()).unwrap_or_else(|| answer_in(&life, &method, params))
            }));
            let answer = answer.unwrap_or_else(|_| Err(Error::internal("the adapter panicked")));
            write(&out, &id, answer);
        }));
    }

    fn hello(&mut self, params: &Value) -> Answer {
        let asked = params.get("protocol").and_then(Value::as_f64);
        if asked != Some(super::VERSION as f64) {
            let name = &self.adapter.hello.adapter.name;
            let shown = params.get("protocol").map(Value::to_string).unwrap_or_default();
            let message =
                "docattest: the {name} adapter speaks protocol {version}, and the core asked for {shown}";
            let message = message
                .replace("{name}", name)
                .replace("{version}", &super::VERSION.to_string())
                .replace("{shown}", &shown);
            return Err(Error::User(message));
        }
        self.greeted = true;
        serde_json::to_value(&self.adapter.hello).map_err(Error::internal)
    }

    fn open(&mut self, params: Value) -> Answer {
        if !matches!(self.life, Life::NotOpen) {
            return Err(Error::internal("a second open: one process serves one session"));
        }
        self.life = Life::Open((self.adapter.open)(params)?);
        Ok(json!({}))
    }

    /// Waits for every request sent before it, then releases the session.
    fn close(&mut self) -> Answer {
        if !matches!(self.life, Life::Open(_)) {
            return Err(Error::internal("close needs a session, and none is open"));
        }
        self.settle();
        self.close_session();
        Ok(json!({}))
    }

    fn settle(&mut self) {
        for handle in self.pending.drain(..) {
            let _ = handle.join();
        }
    }

    fn close_session(&mut self) {
        if let Life::Open(session) = &self.life {
            session.close();
            self.life = Life::Closed;
        }
    }
}

/// Answers a method of the session, or says why there is no answer.
fn answer_in(life: &Life, method: &str, params: Value) -> Answer {
    let unanswered = match life {
        Life::Open(session) => match session.call(method, params) {
            Some(answer) => return answer,
            None => format!("unknown method \"{method}\""),
        },
        Life::Closed => format!("\"{method}\" after close: the session is closed"),
        Life::NotOpen => format!("\"{method}\" needs a session, and none is open"),
    };
    Err(Error::Internal(unanswered))
}

fn write<W: Write>(out: &Mutex<W>, id: &Value, answer: Answer) {
    let response = match answer {
        Ok(result) => json!({ "id": id, "result": result }),
        Err(Error::User(message)) => json!({ "id": id, "error": { "kind": "user", "message": message } }),
        Err(Error::Internal(message)) => {
            json!({ "id": id, "error": { "kind": "internal", "message": message } })
        }
    };
    let mut out = out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = writeln!(out, "{response}");
    let _ = out.flush();
}
