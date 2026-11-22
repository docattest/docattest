//! The language adapter, driven as a child process through PROTOCOL.md.
//!
//! The core holds every language-neutral rule; an adapter answers only what needs its
//! language's compiler or runtime. [`Adapter::launch`] finds and greets it; the session
//! methods send requests that may overlap, and each caller waits for the answers in a fixed
//! order, so which error is reported never depends on timing.

pub(crate) mod builtin;
pub(crate) mod client;
mod command;
#[cfg(test)]
pub(crate) mod fake;
pub(crate) mod language;
pub(crate) mod sources;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::api::errors::{Error, Result};
use crate::config::NONE_ADAPTER;
use builtin::BuiltinNone;
pub(crate) use client::{Client, Pending};
pub(crate) use language::{DocProse, ExpressionShape, Language};
pub(crate) use sources::SourceFiles;

use crate::files::ignored::Ignored;
use command::{adapter_variable, find, from_dir};

/// The protocol version this core speaks.
pub const PROTOCOL: u64 = 1;

/// How the core talks to an adapter: either a child process or the built-in `"none"` backend.
enum Backend {
    Process(Client),
    Builtin(Box<Mutex<BuiltinNone>>),
}

impl Backend {
    fn send(&self, method: &str, params: Value) -> Pending {
        match self {
            Backend::Process(client) => client.send(method, params),
            Backend::Builtin(builtin) => {
                let mut guard = builtin.lock().unwrap_or_else(|e| e.into_inner());
                Pending::ready(guard.call(method, params))
            }
        }
    }

    fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.send(method, params).wait()
    }

    fn finish(self) -> Result<()> {
        match self {
            Backend::Process(client) => client.finish(),
            Backend::Builtin(_) => Ok(()),
        }
    }
}

/// An adapter that has been started, its greeting on the way.
pub(crate) struct Starting {
    backend: Backend,
    greeting: Pending,
    name: String,
}

/// An adapter that has greeted: what it said of its language is known.
pub(crate) struct Adapter {
    backend: Backend,
    /// Info-string languages whose fenced blocks the adapter runs, lower case.
    pub block_languages: Vec<String>,
    /// Which project files are source files of the adapter's language.
    pub source_files: SourceFiles,
    /// The files and directories that are never the project's own, at any depth.
    pub ignored: Ignored,
    pub capabilities: Vec<String>,
    /// Info-string words of the adapter's own, passed on with the example that carries them.
    pub info_words: Vec<String>,
    /// The language's words for what the core's sentences name.
    pub vocabulary: Vocabulary,
    name: String,
}

/// The list of strings `hello` must give under `key`.
fn required_words(hello: &Value, key: &str) -> std::result::Result<Vec<String>, String> {
    hello
        .get(key)
        .and_then(Value::as_array)
        .and_then(|items| items.iter().map(|n| n.as_str().map(str::to_string)).collect())
        .ok_or_else(|| format!("\"{key}\" must be an array of strings"))
}

/// The list of strings `hello` may give under `key`: none when it leaves it out.
fn optional_words(hello: &Value, key: &str) -> std::result::Result<Vec<String>, String> {
    if hello.get(key).is_none() { Ok(Vec::new()) } else { required_words(hello, key) }
}

/// Whether more requests will follow the greeting whatever it says (`hello.warm`): the
/// adapter may then get ready for them, loading its compiler, as soon as it has greeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    /// Requests will follow.
    Warm,
    /// The greeting alone may settle the command.
    Cold,
}

impl Starting {
    /// Wait for the answer to `hello`; nothing else is sent before it. What the adapter said
    /// about its language is known from here on.
    pub fn greeted(self) -> Result<Adapter> {
        let hello = self.greeting.wait()?;
        let greeting = Greeting::read(&hello)
            .map_err(|why| Error::internal(format!("the {} adapter's hello: {why}", self.name)))?;
        let Greeting { block_languages, capabilities, source_files, ignored, info_words, vocabulary } =
            greeting;
        Ok(Adapter {
            block_languages,
            capabilities,
            source_files,
            ignored,
            info_words,
            vocabulary,
            backend: self.backend,
            name: self.name,
        })
    }
}

/// What an adapter's greeting says of its language, as the core reads it ([`Adapter`] keeps
/// each part under the same name).
pub(crate) struct Greeting {
    pub block_languages: Vec<String>,
    pub capabilities: Vec<String>,
    pub source_files: SourceFiles,
    pub ignored: Ignored,
    pub info_words: Vec<String>,
    pub vocabulary: Vocabulary,
}

impl Greeting {
    /// The greeting `hello` holds, or why the core refuses it. The conformance suite holds
    /// every adapter's greeting to this too, so what conforms is what the core accepts.
    pub(crate) fn read(hello: &Value) -> std::result::Result<Greeting, String> {
        let required = |key: &str| required_words(hello, key);
        let optional = |key: &str| optional_words(hello, key);
        let lower = |words: Vec<String>| words.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>();
        Ok(Greeting {
            block_languages: required("blockLanguages").map(lower)?,
            capabilities: required("capabilities")?,
            source_files: SourceFiles::from_hello(hello.get("sourceFiles"))?,
            ignored: Ignored::new(&required("ignoredNames")?, &optional("ignoredMarkers")?)?,
            info_words: optional("infoWords").and_then(info_words)?,
            vocabulary: Vocabulary::from_hello(hello.get("vocabulary"))?,
        })
    }
}

/// Where the command that starts an adapter comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Launch {
    /// The launcher's variable, `DOCATTEST_ADAPTER_<NAME>`, else the PATH.
    Found,
    /// A config's `adapterCommand`: the adapter starts in `dir`, the directory of that
    /// config, and its words are read from there.
    Given { words: Vec<String>, dir: PathBuf },
}

impl Launch {
    /// How a `members` entry starts its adapter: with its `adapterCommand`, read from `dir`,
    /// the directory of the config that lists it; without one, found as every adapter is.
    pub fn of_member(adapter_command: Option<&[String]>, dir: &Path) -> Launch {
        match adapter_command {
            Some(words) => Launch::Given { words: words.to_vec(), dir: dir.to_path_buf() },
            None => Launch::Found,
        }
    }

    /// The variable that starts the same adapter for `name` in a docattest run inside this
    /// one (a console block's `docattest`), when that run would not find it by itself: None
    /// for an adapter the launcher's variable or the PATH found, since the inner run finds
    /// it the same way. The variable has no working directory, so the command changes into
    /// `dir` first, as [`Adapter::launch`] starts it there.
    pub fn variable(&self, name: &str) -> Option<(String, String)> {
        let Launch::Given { words, dir } = self else { return None };
        let in_dir = ["/bin/sh", "-c", "cd \"$0\" && exec \"$@\""].map(String::from);
        let argv: Vec<String> = in_dir
            .into_iter()
            .chain([dir.to_string_lossy().into_owned()])
            .chain(from_dir(words, dir))
            .collect();
        Some((adapter_variable(name), Value::from(argv).to_string()))
    }
}

impl Adapter {
    /// Start the adapter for `name` as `launch` says, sending `hello`. The greeting's answer
    /// is awaited by [`Starting::greeted`].
    pub fn launch(name: &str, launch: &Launch, start: Start) -> Result<Starting> {
        if name == NONE_ADAPTER && matches!(launch, Launch::Found) {
            let backend = Backend::Builtin(Box::new(Mutex::new(BuiltinNone::default())));
            let greeting = backend.send("hello", json!({ "protocol": PROTOCOL }));
            return Ok(Starting { backend, greeting, name: name.to_string() });
        }
        match launch {
            Launch::Found => Adapter::spawn(&find(name)?, None, name, start),
            Launch::Given { words, dir } => Adapter::spawn(&from_dir(words, dir), Some(dir), name, start),
        }
    }

    /// Start the adapter from its command line, sending `hello`.
    #[cfg(test)]
    pub fn start_command(argv: &[String], name: &str, start: Start) -> Result<Starting> {
        Adapter::spawn(argv, None, name, start)
    }

    fn spawn(argv: &[String], dir: Option<&Path>, name: &str, start: Start) -> Result<Starting> {
        let backend = Backend::Process(Client::spawn(argv, dir, name)?);
        let hello = match start {
            Start::Warm => json!({ "protocol": PROTOCOL, "warm": true }),
            Start::Cold => json!({ "protocol": PROTOCOL }),
        };
        let greeting = backend.send("hello", hello);
        Ok(Starting { backend, greeting, name: name.to_string() })
    }

    /// Whether the adapter implements the optional method `capability`.
    pub fn can(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// The adapter's name, as the config gives it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Send a request (after the greeting).
    pub fn send(&self, method: &str, params: Value) -> Pending {
        self.backend.send(method, params)
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.backend.call(method, params)
    }

    /// Close the session that was opened, then let the adapter exit.
    pub fn finish_session(self) -> Result<()> {
        self.backend.call("close", json!({}))?;
        self.backend.finish()
    }

    /// Let the adapter exit; no session was opened.
    pub fn finish(self) -> Result<()> {
        self.backend.finish()
    }
}

/// An info-string word of the adapter's own is one word, or `key=` for an attribute.
fn info_words(words: Vec<String>) -> std::result::Result<Vec<String>, String> {
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| crate::text::is_word(c) || c == '-');
    match words.iter().find(|w| !word(w.strip_suffix('=').unwrap_or(w))) {
        Some(bad) => Err(format!("\"infoWords\" has {bad:?}, which is neither a word nor \"key=\"")),
        None => Ok(words),
    }
}

/// The language's own words, for the sentences the core composes (`hello.vocabulary`). Every
/// one is optional, and the default says the same thing in no language's terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Vocabulary {
    /// What stands for an enclosing title that cannot be cited, in a test's path.
    pub untitled: String,
    /// What a test file with no test to cite is said to declare.
    pub no_tests: String,
}

impl Default for Vocabulary {
    fn default() -> Vocabulary {
        Vocabulary { untitled: "(untitled)".into(), no_tests: "it declares no test".into() }
    }
}

impl Vocabulary {
    fn from_hello(value: Option<&Value>) -> std::result::Result<Vocabulary, String> {
        let mut vocabulary = Vocabulary::default();
        let Some(value) = value else { return Ok(vocabulary) };
        let Value::Object(words) = value else {
            return Err("\"vocabulary\" must be an object of strings".into());
        };
        for (key, word) in words {
            let Some(word) = word.as_str() else {
                return Err(format!("\"vocabulary.{key}\" must be a string"));
            };
            match key.as_str() {
                "untitled" => vocabulary.untitled = word.to_string(),
                "noTests" => vocabulary.no_tests = word.to_string(),
                // A word this core has no sentence for: a newer adapter, an older core.
                _ => {}
            }
        }
        Ok(vocabulary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_has_a_neutral_default_for_every_word() {
        let read = |v: Value| Vocabulary::from_hello(Some(&v));
        assert_eq!(Vocabulary::from_hello(None).unwrap(), Vocabulary::default());
        let said = read(json!({ "untitled": "(computed title)", "later": "x" })).unwrap();
        assert_eq!(said.untitled, "(computed title)");
        assert_eq!(said.no_tests, "it declares no test");
        assert!(read(json!({ "noTests": 1 })).is_err());
        assert!(read(json!([])).is_err());
    }

    #[test]
    fn capabilities_are_read_as_the_protocol_spells_them() {
        let hello = json!({ "blockLanguages": ["TS"], "capabilities": ["tests", "runTests"],
            "sourceFiles": { "include": [], "exclude": [] }, "ignoredNames": [] });
        let greeting = Greeting::read(&hello).unwrap();
        assert_eq!(greeting.capabilities, ["tests", "runTests"]);
        assert_eq!(greeting.block_languages, ["ts"]);
    }

    #[test]
    fn info_words_are_words_or_attribute_keys() {
        assert!(info_words(vec!["continue".into(), "session=".into(), "no-run".into()]).is_ok());
        assert!(info_words(vec!["a b".into()]).is_err());
        assert!(info_words(vec!["=".into()]).is_err());
    }
}
