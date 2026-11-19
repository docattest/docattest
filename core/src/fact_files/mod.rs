//! The project files facts read: `manifest("<path>")` gives a file's data, parsed by its
//! format (`src/manifests/`), and `manifestField("<path>", "<keys>")` the value at a path in
//! it; `file("<path>")` gives its text, and `matched("<path>", "<regex>")` what a regex
//! matches in it ([`matched`]). The core reads them when the session opens, works out what each call
//! gives, and gives that to the adapter, as it gives it the snapshots.
//!
//! Only the calls facts write out with string literals are read ([`calls`]). The core never
//! evaluates an expression, and the adapter cannot ask the core for a file while it
//! evaluates one, so the session carries everything a fact may read, and a path built at
//! run time is not one.

pub mod calls;
mod matched;
pub mod mutants;

use std::path::{Component, Path};

use serde_json::{Map, Value};

use crate::api::attestation::{Attestation, Body};
use crate::attestations::field::FieldPath;
use crate::files::project_files::ProjectFiles;
use crate::files::source::{normalise, posix_normalize};
use crate::manifests::format::parse_manifest;
use calls::{Call, Reader, named_in};
use matched::matched_text;

/// One call a fact makes: the project file it reads (None when the path leaves the
/// project), and what the call gives or why it gives nothing.
#[derive(Debug, Clone, PartialEq)]
struct Named {
    call: Call,
    file: Option<String>,
    read: Result<Value, String>,
}

/// Every call that reads a file the facts of a project make, each once, in the order they
/// are first made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactFiles {
    named: Vec<Named>,
}

/// A fact that reads a changed file, and how it reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading<'a> {
    pub reader: Reader,
    pub file: String,
    pub fact: &'a Attestation,
}

/// Read every file the facts in `attestations` name, for each call that names it.
pub fn load_fact_files(files: &dyn ProjectFiles, attestations: &[Attestation]) -> FactFiles {
    let mut named: Vec<Named> = Vec::new();
    for call in attestations.iter().filter_map(expression_of).flat_map(named_in) {
        if !named.iter().any(|n| n.call == call) {
            named.push(read(files, call));
        }
    }
    FactFiles { named }
}

impl FactFiles {
    /// What `open` gives the adapter of what facts read, each by the arguments as facts
    /// write them: `manifests` always; `texts`, `matches` and `manifestFields` only when a
    /// fact reads one, so that an adapter that predates one is sent nothing new for a
    /// project that does not use it.
    pub fn session_params(&self) -> Map<String, Value> {
        let (mut manifests, mut texts) = (Map::new(), Map::new());
        let (mut matches, mut fields) = (Vec::new(), Vec::new());
        for Named { call, read, .. } in &self.named {
            let Ok(value) = read else { continue };
            let value = value.clone();
            match call {
                Call::Manifest { path } => {
                    manifests.insert(path.clone(), value);
                }
                Call::File { path } => {
                    texts.insert(path.clone(), value);
                }
                Call::Matched { path, pattern } => by_path(&mut matches, path, pattern, value),
                Call::ManifestField { path, keys } => by_path(&mut fields, path, keys, value),
            }
        }
        let mut params = Map::new();
        params.insert("manifests".into(), Value::Object(manifests));
        let optional =
            [("texts", texts), ("matches", by_path_map(matches)), ("manifestFields", by_path_map(fields))];
        for (name, read) in optional.into_iter().filter(|(_, read)| !read.is_empty()) {
            params.insert(name.into(), Value::Object(read));
        }
        params
    }

    /// Make each fact that makes a call that gives nothing an `error` that never runs,
    /// saying which file and why.
    pub fn refuse_unreadable(&self, attestations: &mut [Attestation]) {
        for a in attestations.iter_mut().filter(|a| a.ok()) {
            let Some(expression) = expression_of(a) else { continue };
            let unreadable = named_in(expression).find_map(|call| self.problem(&call).map(str::to_string));
            if let Some(problem) = unreadable {
                a.error = Some(problem);
            }
        }
    }

    /// The facts that read a file among `changed` (project-relative files): a change to the
    /// file can make them untrue.
    pub fn readers<'a>(&self, attestations: &'a [Attestation], changed: &[String]) -> Vec<Reading<'a>> {
        let mut out: Vec<Reading> = Vec::new();
        for fact in attestations {
            let Some(expression) = expression_of(fact) else { continue };
            for call in named_in(expression) {
                let Some(file) = self.named(&call).and_then(|n| n.file.as_deref()) else {
                    continue;
                };
                let reading = Reading { reader: call.reader(), file: file.to_string(), fact };
                if changed.iter().any(|c| c == file) && !out.contains(&reading) {
                    out.push(reading);
                }
            }
        }
        out
    }

    fn problem(&self, call: &Call) -> Option<&str> {
        self.named(call)?.read.as_ref().err().map(String::as_str)
    }

    fn named(&self, call: &Call) -> Option<&Named> {
        self.named.iter().find(|n| n.call == *call)
    }
}

/// What calls with two arguments give, by path, then by argument, the paths in the order
/// they are first named.
type ByPath = Vec<(String, Map<String, Value>)>;

/// Put `value` in `read` under `path`, then under `argument`.
fn by_path(read: &mut ByPath, path: &str, argument: &str, value: Value) {
    let at = read.iter().position(|(named, _)| named == path).unwrap_or_else(|| {
        read.push((path.to_string(), Map::new()));
        read.len() - 1
    });
    read[at].1.insert(argument.to_string(), value);
}

/// `read` as `open` gives it: an object of objects.
fn by_path_map(read: ByPath) -> Map<String, Value> {
    read.into_iter().map(|(path, by_argument)| (path, Value::Object(by_argument))).collect()
}

fn expression_of(attestation: &Attestation) -> Option<&str> {
    match &attestation.body {
        Body::Fact(fact) => Some(&fact.expression),
        _ => None,
    }
}

fn read(files: &dyn ProjectFiles, call: Call) -> Named {
    let file = inside_project(call.path());
    let read = match &file {
        None => Err(format!("{} \"{}\" is outside the project", call.function(), call.path())),
        Some(file) => read_file(files, file, &call),
    };
    Named { call, file, read }
}

/// What `call` gives of `file`, a project file; or why it gives nothing.
fn read_file(files: &dyn ProjectFiles, file: &str, call: &Call) -> Result<Value, String> {
    let (name, written) = (call.function(), call.path());
    let because = |why: String| format!("{name} \"{written}\": {why}");
    if !files.exists(file) {
        return Err(because("no such file".to_string()));
    }
    let text = utf8_text(files, file).map_err(because)?;
    match call {
        Call::Manifest { .. } => parse_manifest(file, &text).map_err(because),
        Call::File { .. } => Ok(Value::String(text)),
        Call::Matched { pattern, .. } => matched_text(&text, pattern).map(Value::String).map_err(because),
        Call::ManifestField { keys, .. } => {
            let data = parse_manifest(file, &text).map_err(because)?;
            let path = FieldPath::parse(keys).map_err(because)?;
            let value =
                path.value_in(&data).map_err(|why| format!("{name} \"{written}\" has no {keys}: {why}"))?;
            Ok(value.clone())
        }
    }
}

/// The text of a file a fact reads. It crosses the protocol as JSON, which holds only
/// Unicode, and a byte that is not UTF-8 would reach the fact as U+FFFD and make it pass or
/// fail for a reason no reader sees; so such a file is refused.
fn utf8_text(files: &dyn ProjectFiles, file: &str) -> Result<String, String> {
    let bytes = files.read_bytes(file)?;
    String::from_utf8(bytes)
        .map_err(|e| format!("not UTF-8 text (invalid byte at offset {})", e.utf8_error().valid_up_to()))
}

/// The project-relative file a path names, or None when it leaves the project: it is
/// absolute, or climbs above the root.
fn inside_project(written: &str) -> Option<String> {
    let file = posix_normalize(&normalise(written));
    let inside = Path::new(&file).components().all(|c| matches!(c, Component::Normal(_)));
    inside.then_some(file)
}

#[cfg(test)]
mod tests;
