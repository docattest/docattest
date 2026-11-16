//! The protocol's data types. Optional fields are left out, never null.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The greeting: what is fixed for an adapter, whatever the project.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub protocol: u64,
    pub adapter: AdapterName,
    pub block_languages: Vec<String>,
    pub source_files: SourceFiles,
    pub ignored_names: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub info_words: Vec<String>,
    pub vocabulary: Vocabulary,
}

/// An adapter's name and version.
#[derive(Clone, Debug, Serialize)]
pub struct AdapterName {
    pub name: String,
    pub version: String,
}

/// Globs for the project paths that are source files of the language.
#[derive(Clone, Debug, Serialize)]
pub struct SourceFiles {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// A method beyond the required ones that an adapter answers.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    Tests,
    Observables,
    Affected,
}

/// The language's words for what the core's sentences name.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    pub no_tests: String,
}

/// An ExampleAttestation: a fenced block, or an example in a doc comment.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Example {
    pub id: String,
    pub page: String,
    pub line: usize,
    pub claim: String,
    pub kind: String,
    pub lang: String,
    pub mode: String,
    pub network: bool,
    pub code: String,
    pub weak: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A FactAttestation, as far as the adapter reads it: an expression over a surface.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Fact {
    pub id: String,
    pub expression: String,
    pub surface: String,
}

/// One row of a cases table.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CasesRow {
    pub line: usize,
    pub name: String,
    pub values: BTreeMap<String, String>,
}

/// A CasesAttestation: a table run row by row by a runner of the setup module.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Cases {
    pub page: String,
    pub line: usize,
    pub runner: String,
    pub header: Vec<String>,
    pub rows: Vec<CasesRow>,
}

/// Any of the four kinds of attestation, with the fields `symbols` reads of each.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Attestation {
    pub id: String,
    pub page: String,
    pub line: usize,
    pub kind: String,
    pub code: String,
    pub owner: Option<String>,
    pub expression: String,
    pub surface: String,
    pub runner: String,
    pub file: String,
    pub title: String,
    pub describes: Vec<String>,
}

/// A reason an example does not compile.
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub id: String,
    pub message: String,
    pub line: usize,
}

/// A value as the runtime observed it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Observation {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// The outcome of evaluating one fact expression.
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Evaluation {
    /// The evaluation of a fact that has no value.
    pub fn failed(message: impl Into<String>) -> Self {
        Evaluation { ok: false, value: None, message: Some(message.into()) }
    }

    /// The evaluation of a fact that has a value.
    pub fn of(value: Observation) -> Self {
        Evaluation { ok: true, value: Some(value), message: None }
    }
}

/// A surface symbol an attestation exercises.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct SymbolRef {
    pub name: String,
    pub surface: String,
}

/// What kind of attestation a verdict is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Example,
    Fact,
    Cases,
    Snapshot,
    Test,
}

/// How an attestation fared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Fail,
    Error,
    Skipped,
}

/// The verdict on one example, or on one row of a cases table.
#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    pub page: String,
    pub line: usize,
    pub kind: Kind,
    pub status: Status,
    pub symbols: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    pub message: String,
}

/// A test, or something that may be cited by mistake for one, that a test file declares.
#[derive(Clone, Debug, Serialize)]
pub struct Declared {
    pub title: String,
    pub describes: Vec<String>,
    pub line: usize,
    /// What it is when it is not a test, with its article.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// A value a surface offers, with the fact expression that reads it.
#[derive(Clone, Debug, Serialize)]
pub struct Observable {
    pub surface: String,
    pub expression: String,
    pub path: Vec<String>,
    pub value: Observation,
}

/// A file by path and text, as the questions about text receive it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SourceText {
    pub page: String,
    pub text: String,
}

/// One doc comment as prose, with its first and last line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DocProse {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// A fact expression with its names taken out.
#[derive(Clone, Debug, Serialize)]
pub struct ExpressionShape {
    pub shape: String,
    pub names: Vec<String>,
}
