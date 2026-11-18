//! The project files facts read: `manifest("<path>")` gives a file's data, parsed by its
//! format (`src/manifests/`), and `file("<path>")` its text. The core reads them when the
//! session opens and gives them to the adapter, as it gives it the snapshots.
//!
//! Only the paths facts write out as a string literal are read ([`calls`]). The core never
//! evaluates an expression, and the adapter cannot ask the core for a file while it
//! evaluates one, so the session carries every file a fact may read, and a path built at
//! run time is not one.

pub mod calls;

use std::path::{Component, Path};

use serde_json::{Map, Value};

use crate::api::attestation::{Attestation, Body};
use crate::files::project_files::ProjectFiles;
use crate::files::source::{normalise, posix_normalize};
use crate::manifests::format::parse_manifest;
use calls::{Reader, named_in};

/// One file a fact names: how it is read, the path as the fact writes it, the project file
/// it is (None when the path leaves the project), and what was read or why nothing was.
#[derive(Debug, Clone, PartialEq)]
struct Named {
    reader: Reader,
    written: String,
    file: Option<String>,
    read: Result<Value, String>,
}

/// Every file the facts of a project name, each once for each function that reads it, in
/// the order they are first named.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactFiles {
    named: Vec<Named>,
}

/// A fact that reads a changed file, and the function it reads it with.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading<'a> {
    pub reader: Reader,
    pub file: String,
    pub fact: &'a Attestation,
}

/// Read every file the facts in `attestations` name.
pub fn load_fact_files(files: &dyn ProjectFiles, attestations: &[Attestation]) -> FactFiles {
    let mut named: Vec<Named> = Vec::new();
    for (reader, written) in attestations.iter().filter_map(expression_of).flat_map(named_in) {
        if !named.iter().any(|n| n.reader == reader && n.written == written) {
            named.push(read(files, reader, written));
        }
    }
    FactFiles { named }
}

impl FactFiles {
    /// What `reader` read of every file it could, by the path as facts write it: what
    /// `open` gives the adapter.
    pub fn read_with(&self, reader: Reader) -> Map<String, Value> {
        let read = self.named.iter().filter(|n| n.reader == reader);
        read.filter_map(|n| Some((n.written.clone(), n.read.as_ref().ok()?.clone()))).collect()
    }

    /// Make each fact that names a file that cannot be read an `error` that never runs,
    /// saying which file and why.
    pub fn refuse_unreadable(&self, attestations: &mut [Attestation]) {
        for a in attestations.iter_mut().filter(|a| a.ok()) {
            let Some(expression) = expression_of(a) else { continue };
            let unreadable = named_in(expression).find_map(|(reader, written)| self.problem(reader, written));
            if let Some(problem) = unreadable {
                a.error = Some(problem.to_string());
            }
        }
    }

    /// The facts that read a file among `changed` (project-relative files): a change to the
    /// file can make them untrue.
    pub fn readers<'a>(&self, attestations: &'a [Attestation], changed: &[String]) -> Vec<Reading<'a>> {
        let mut out: Vec<Reading> = Vec::new();
        for fact in attestations {
            let Some(expression) = expression_of(fact) else { continue };
            for (reader, written) in named_in(expression) {
                let Some(file) = self.named(reader, written).and_then(|n| n.file.as_deref()) else {
                    continue;
                };
                let reading = Reading { reader, file: file.to_string(), fact };
                if changed.iter().any(|c| c == file) && !out.contains(&reading) {
                    out.push(reading);
                }
            }
        }
        out
    }

    fn problem(&self, reader: Reader, written: &str) -> Option<&str> {
        self.named(reader, written)?.read.as_ref().err().map(String::as_str)
    }

    /// The file a fact reads with `reader` as `written`.
    fn named(&self, reader: Reader, written: &str) -> Option<&Named> {
        self.named.iter().find(|n| n.reader == reader && n.written == written)
    }
}

fn expression_of(attestation: &Attestation) -> Option<&str> {
    match &attestation.body {
        Body::Fact(fact) => Some(&fact.expression),
        _ => None,
    }
}

fn read(files: &dyn ProjectFiles, reader: Reader, written: &str) -> Named {
    let file = inside_project(written);
    let name = reader.name();
    let read = match &file {
        None => Err(format!("{name} \"{written}\" is outside the project")),
        Some(file) if !files.exists(file) => Err(format!("{name} \"{written}\": no such file")),
        Some(file) => utf8_text(files, file)
            .and_then(|text| match reader {
                Reader::Manifest => parse_manifest(file, &text),
                Reader::File => Ok(Value::String(text)),
            })
            .map_err(|why| format!("{name} \"{written}\": {why}")),
    };
    Named { reader, written: written.to_string(), file, read }
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
