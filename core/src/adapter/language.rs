//! What the core asks a language (PROTOCOL.md §3). The core never reads source code and
//! knows no language's syntax: which files are code, which names are never the project's
//! own, what a doc comment says, whether an example asserts, which symbol a code span
//! names, what shape an expression has and whether a name is still in the code are all the
//! adapter's to say.
//!
//! Every question takes many items and is one round trip. [`Language`] is implemented by the
//! adapter process ([`super::Adapter`]); the unit tests implement it with a stand-in.

use serde_json::{Value, json};

use super::{Adapter, SourceFiles};
use crate::api::attestation::{Attestation, example_from_adapter};
use crate::api::errors::{Error, Result};

/// The prose of one doc comment: its description, as a claim is written, and its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocProse {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// A fact expression with its names taken out. Two expressions with the same shape differ in
/// nothing but their names; a shape is only ever compared with another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionShape {
    pub shape: String,
    pub names: Vec<String>,
}

/// A language, as the comparison and the planner see it.
pub trait Language {
    /// The adapter's name, as the config gives it.
    fn name(&self) -> &str;
    /// Whether the adapter reads the tests a test file declares, so citations can be checked.
    fn reads_tests(&self) -> bool;
    /// Info-string languages whose fenced blocks the adapter runs, lower case.
    fn block_languages(&self) -> &[String];
    /// Info-string words of the adapter's own, passed on with the example that has them.
    fn info_words(&self) -> &[String];
    /// Which project files are source files of the language.
    fn source_files(&self) -> &SourceFiles;
    /// The doc-comment examples of source files (page and text), in the adapter's order.
    fn doc_comments(&self, files: &[(&str, &str)]) -> Result<Vec<Attestation>>;
    /// Whether each example (block language and code) asserts anything.
    fn assertions(&self, examples: &[(&str, &str)]) -> Result<Vec<bool>>;
    /// [`Language::assertions`] and [`Language::doc_comments`] together; an empty list is
    /// not asked about. An adapter may answer both at once.
    fn assertions_and_doc_comments(
        &self,
        examples: &[(&str, &str)],
        files: &[(&str, &str)],
    ) -> Result<(Vec<bool>, Vec<Attestation>)> {
        let asserting = if examples.is_empty() { Vec::new() } else { self.assertions(examples)? };
        let commented = if files.is_empty() { Vec::new() } else { self.doc_comments(files)? };
        Ok((asserting, commented))
    }
    /// The doc comments of each source file (page and text) as prose, in order.
    fn doc_prose(&self, files: &[(&str, &str)]) -> Result<Vec<Vec<DocProse>>>;
    /// For each inline code span, the one of `symbols` it names.
    fn mentions(&self, spans: &[&str], symbols: &[&str]) -> Result<Vec<Option<String>>>;
    /// The shape and the names of each fact expression.
    fn expressions(&self, expressions: &[&str]) -> Result<Vec<ExpressionShape>>;
    /// Which of `names` the open project's code still holds: `files` (project paths) and
    /// the files its surfaces are read from. Needs the session.
    fn in_code(&self, names: &[&str], files: &[String]) -> Result<Vec<String>>;
}

fn pairs(files: &[(&str, &str)], first: &str, second: &str) -> Vec<Value> {
    files.iter().map(|(a, b)| json!({ first: a, second: b })).collect()
}

/// An answer that must be a list with one entry for each item asked about.
fn list(method: &str, answer: Value, asked: usize) -> Result<Vec<Value>> {
    match answer {
        Value::Array(items) if items.len() == asked => Ok(items),
        _ => Err(Error::internal(format!("{method} did not answer one entry for each of {asked} items"))),
    }
}

/// The request for `assertions`.
pub fn assertions_params(examples: &[(&str, &str)]) -> Value {
    json!({ "examples": pairs(examples, "lang", "code") })
}

/// The answer to `assertions`.
pub fn assertions_answer(answer: Value, asked: usize) -> Result<Vec<bool>> {
    list("assertions", answer, asked)?
        .iter()
        .map(|v| {
            v.as_bool().ok_or_else(|| Error::internal("assertions answered something that is not a boolean"))
        })
        .collect()
}

/// The request for `docComments`.
pub fn doc_comments_params(files: &[(&str, &str)]) -> Value {
    json!({ "files": pairs(files, "page", "text") })
}

/// The answer to `docComments`.
pub fn doc_comments_answer(answer: Value) -> Result<Vec<Attestation>> {
    let Value::Array(items) = answer else {
        return Err(Error::internal("docComments did not answer a list"));
    };
    items
        .iter()
        .map(|item| {
            example_from_adapter(item)
                .ok_or_else(|| Error::internal("docComments answered something that is not an example"))
        })
        .collect()
}

fn prose(item: &Value) -> Option<DocProse> {
    let line = |key: &str| item.get(key).and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok());
    Some(DocProse { text: item.get("text")?.as_str()?.to_string(), start: line("start")?, end: line("end")? })
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value.as_array()?.iter().map(|v| v.as_str().map(str::to_string)).collect()
}

impl Language for Adapter {
    fn name(&self) -> &str {
        Adapter::name(self)
    }

    fn reads_tests(&self) -> bool {
        self.can("tests")
    }

    fn block_languages(&self) -> &[String] {
        &self.block_languages
    }

    fn info_words(&self) -> &[String] {
        &self.info_words
    }

    fn source_files(&self) -> &SourceFiles {
        &self.source_files
    }

    fn doc_comments(&self, files: &[(&str, &str)]) -> Result<Vec<Attestation>> {
        doc_comments_answer(self.call("docComments", doc_comments_params(files))?)
    }

    fn assertions(&self, examples: &[(&str, &str)]) -> Result<Vec<bool>> {
        assertions_answer(self.call("assertions", assertions_params(examples))?, examples.len())
    }

    /// Both requests are sent before either answer is awaited.
    fn assertions_and_doc_comments(
        &self,
        examples: &[(&str, &str)],
        files: &[(&str, &str)],
    ) -> Result<(Vec<bool>, Vec<Attestation>)> {
        let asserting = (!examples.is_empty()).then(|| self.send("assertions", assertions_params(examples)));
        let commented = (!files.is_empty()).then(|| self.send("docComments", doc_comments_params(files)));
        let asserting = match asserting {
            Some(answer) => assertions_answer(answer.wait()?, examples.len())?,
            None => Vec::new(),
        };
        let commented = match commented {
            Some(answer) => doc_comments_answer(answer.wait()?)?,
            None => Vec::new(),
        };
        Ok((asserting, commented))
    }

    fn doc_prose(&self, files: &[(&str, &str)]) -> Result<Vec<Vec<DocProse>>> {
        let answer = self.call("docProse", json!({ "files": pairs(files, "page", "text") }))?;
        list("docProse", answer, files.len())?
            .iter()
            .map(|file| {
                file.as_array()
                    .and_then(|comments| comments.iter().map(prose).collect())
                    .ok_or_else(|| Error::internal("docProse answered something that is not a doc comment"))
            })
            .collect()
    }

    fn mentions(&self, spans: &[&str], symbols: &[&str]) -> Result<Vec<Option<String>>> {
        let answer = self.call("mentions", json!({ "spans": spans, "symbols": symbols }))?;
        list("mentions", answer, spans.len())?
            .iter()
            .map(|v| match v {
                Value::Null => Ok(None),
                Value::String(name) => Ok(Some(name.clone())),
                _ => Err(Error::internal("mentions answered something that is not a name")),
            })
            .collect()
    }

    fn expressions(&self, expressions: &[&str]) -> Result<Vec<ExpressionShape>> {
        let answer = self.call("expressions", json!({ "expressions": expressions }))?;
        list("expressions", answer, expressions.len())?
            .iter()
            .map(|v| {
                let shape = v.get("shape").and_then(Value::as_str)?.to_string();
                Some(ExpressionShape { shape, names: v.get("names").and_then(strings)? })
            })
            .map(|shape| {
                shape.ok_or_else(|| Error::internal("expressions answered something that is not a shape"))
            })
            .collect()
    }

    fn in_code(&self, names: &[&str], files: &[String]) -> Result<Vec<String>> {
        let answer = self.call("inCode", json!({ "names": names, "files": files }))?;
        strings(&answer).ok_or_else(|| Error::internal("inCode did not answer a list of names"))
    }
}
