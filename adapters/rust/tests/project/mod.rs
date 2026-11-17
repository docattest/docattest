//! What the tests share: sessions opened as the core opens them, small cargo projects
//! written to the temporary directory, and the `docattest` command run in a project.

// Each test file is a crate of its own, and none uses all of this module.
#![allow(dead_code)]

use docattest_rust::protocol::{Error, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Opens a session on a project with these surfaces, told what the core tells it, with
/// each key of `replaced` in place of what it would say.
pub fn open_session(root: &Path, surfaces: Value, replaced: Value) -> Arc<dyn Session> {
    let mut told = json!({
        "root": root, "surfaces": surfaces,
        "timeoutMs": 2000, "workers": 2, "network": false, "snapshots": { "data": {} }, "manifests": {}, "examples": [],
    });
    for (key, value) in replaced.as_object().into_iter().flatten() {
        told[key] = value.clone();
    }
    docattest_rust::session::open(told).unwrap_or_else(|_| panic!("the session did not open"))
}

/// What a session answers, or panics with why it did not.
pub fn call(session: &Arc<dyn Session>, method: &str, params: Value) -> Value {
    try_call(session, method, params).unwrap_or_else(|message| panic!("{method}: {message}"))
}

/// What a session answers, or the message it refused with.
pub fn try_call(session: &Arc<dyn Session>, method: &str, params: Value) -> Result<Value, String> {
    match session.call(method, params) {
        Some(Ok(value)) => Ok(value),
        Some(Err(Error::User(message) | Error::Internal(message))) => Err(message),
        None => Err(format!("no method {method}")),
    }
}

/// Symbols as the protocol gives them, as `surface:name` words.
pub fn surface_names(symbols: &Value) -> Vec<String> {
    let symbols = symbols.as_array().unwrap().iter();
    symbols
        .map(|symbol| format!("{}:{}", symbol["surface"].as_str().unwrap(), symbol["name"].as_str().unwrap()))
        .collect()
}

/// The exports a change affects, as `surface:name` words, when a file of the project
/// held `was` at the base where it now holds `now`.
pub fn affected(session: &Arc<dyn Session>, root: &Path, file: &str, now: &str, was: &str) -> Vec<String> {
    let text = std::fs::read_to_string(root.join(file)).unwrap();
    assert!(text.contains(now), "{file} does not hold {now:?}");
    let base = json!({ "base": { file: text.replacen(now, was, 1) }, "changed": [file] });
    surface_names(&call(session, "affected", base))
}

/// A project on disk with a session open on it, removed when the test is done with it.
pub struct Written {
    pub root: PathBuf,
    pub session: Arc<dyn Session>,
}

/// Writes a project named for its test, each file by its path below the root, and gives
/// the root.
pub fn write(test: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("docattest-rust-{}-{test}", std::process::id()));
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

impl Written {
    /// Writes a project named for its test and opens a session on it with these surfaces.
    pub fn open(test: &str, files: &[(&str, &str)], surfaces: Value) -> Written {
        let root = write(test, files);
        let session = open_session(&root, surfaces, json!({}));
        Written { root, session }
    }

    pub fn call(&self, method: &str, params: Value) -> Value {
        call(&self.session, method, params)
    }
}

impl Drop for Written {
    fn drop(&mut self) {
        self.session.close();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Runs the `docattest` command in a project, as a user runs it, with some variables
/// added to its environment, and gives what it printed and its exit code. Nothing says
/// where the adapter is: the command finds it beside itself.
pub fn docattest(root: &Path, args: &[&str], env: &[(&str, &str)]) -> (String, i32) {
    let ran = Command::new(env!("CARGO_BIN_EXE_docattest"))
        .args(args)
        .arg("--root")
        .arg(root)
        .env_remove("DOCATTEST_CORE")
        .env_remove("DOCATTEST_ADAPTER_RUST")
        .env_remove("DOCATTEST_NETWORK")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .envs(env.iter().copied())
        .output()
        .unwrap();
    let code = ran.status.code().unwrap_or(-1);
    assert_ne!(code, 2, "docattest {args:?}: an internal error\n{}", String::from_utf8_lossy(&ran.stderr));
    (String::from_utf8_lossy(&ran.stdout).into_owned(), code)
}
