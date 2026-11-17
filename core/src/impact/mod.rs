//! From a change to the prose it touches.
//!
//! The adapter names the surface exports the change affects; the index adds every
//! attestation of them, and the living pages every code span that names one without proving
//! anything. A changed manifest affects the facts that read it, and a change to a command's
//! files the blocks that run it. Reads only: nothing runs, nothing is written.

pub mod brief;
pub mod commands;
pub mod covered;
pub mod mentions;
pub mod text;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::adapter::Language;
use crate::api::attestation::Kind;
use crate::api::errors::{Error, Result};
use crate::files::project_files::ProjectFiles;
use crate::git::change::git_change;
use crate::manifests::manifest_call;
use crate::project::check::Network;
use crate::project::check::{Reference, build_index, open_session, request_symbols};
use crate::project::state::State;
use crate::text::compare_utf16;

/// One place an affected symbol appears in a living page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactReference {
    pub symbol: String,
    pub page: String,
    pub line: usize,
    pub found: Found,
    /// The sentence an attestation defends, or the sentence holding the mention.
    pub claim: String,
}

/// How a page refers to an affected symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// An attestation of this kind, which docattest runs or compares, proves the claim.
    Proven(Kind),
    /// A test citation defends the claim: docattest checks that the test is still there,
    /// never that it passes, so the claim is not proven.
    Cited,
    /// A code span names the symbol, and nothing proves the sentence.
    Mention,
}

impl Found {
    /// How an attestation of `kind` defends its claim.
    pub fn of(kind: Kind) -> Found {
        if kind == Kind::Test { Found::Cited } else { Found::Proven(kind) }
    }

    /// The attestation's kind, or `mention`.
    pub fn kind(self) -> &'static str {
        match self {
            Found::Proven(kind) => kind.as_str(),
            Found::Cited => Kind::Test.as_str(),
            Found::Mention => "mention",
        }
    }

    pub fn is_proven(self) -> bool {
        matches!(self, Found::Proven(_))
    }

    /// Whether `docattest check` judges the claim: every attestation does, a citation too.
    pub fn is_checked(self) -> bool {
        !matches!(self, Found::Mention)
    }
}

/// The prose a change touches: what `docattest impact` prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impact {
    /// The revision the working tree was compared with.
    pub base: String,
    /// Affected surface exports, sorted.
    pub symbols: Vec<String>,
    /// Changed manifests that facts read, sorted. A reference to one has its call as its
    /// symbol: `manifest("package.json")`.
    pub manifests: Vec<String>,
    /// Commands built from a changed file (the config's `commands`), sorted. A reference to
    /// one, a block that runs it, has its call as its symbol: `$ mytool`.
    pub commands: Vec<String>,
    /// By symbol, then page, then line.
    pub references: Vec<ImpactReference>,
}

impl Impact {
    /// Whether the change reaches no export, manifest or command.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty() && self.manifests.is_empty() && self.commands.is_empty()
    }

    /// The impact as `--json` prints it, keys in the order the recordings hold; `manifests`
    /// only when a manifest changed, `commands` only when a command did.
    pub fn to_json(&self) -> Value {
        let references: Vec<Value> = self
            .references
            .iter()
            .map(|r| {
                json!({
                    "symbol": r.symbol, "page": r.page, "line": r.line,
                    "kind": r.found.kind(), "claim": r.claim, "proven": r.found.is_proven(),
                })
            })
            .collect();
        let mut out = json!({ "base": self.base, "symbols": self.symbols });
        if !self.manifests.is_empty() {
            out["manifests"] = json!(self.manifests);
        }
        if !self.commands.is_empty() {
            out["commands"] = json!(self.commands);
        }
        out["references"] = Value::Array(references);
        out
    }
}

type Index = Vec<(String, Vec<Reference>)>;

/// The working tree's difference from a git revision, as the `affected` request carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Project-relative paths that differ, untracked files included. Sorted.
    pub changed: Vec<String>,
    /// The contents at the revision of every changed path; None where it did not exist.
    pub base: Vec<(String, Option<String>)>,
}

/// How much of the working tree's difference an impact answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Every changed file: `docattest impact`.
    Diff,
    /// One project file, as though no other had changed: `impact --file` after an edit, which
    /// asks what that edit touched, not what the rest of the uncommitted work did.
    File(String),
}

/// Read the change from `reference` to the working tree of the project at `root` with git,
/// within `scope`. It reads only, so it can run on its own thread while the project loads.
pub fn read_change(root: &Path, reference: &str, scope: &Scope) -> Result<Change> {
    let change = match scope {
        Scope::Diff => git_change(root, reference)?,
        Scope::File(file) => git_change(root, reference)?.only(file),
    };
    let base = change.base_contents();
    Ok(Change { changed: change.changed, base })
}

/// What the working tree's difference from the revision `label` touches; `change` is that
/// difference, or why git could not tell. On success a session is open: the caller closes it.
pub fn run_impact(state: &State, network: Network, label: &str, change: Result<Change>) -> Result<Impact> {
    let change = change?;
    if !state.adapter.can("affected") {
        return Err(Error::user(format!(
            "the {} adapter cannot tell what a change affects",
            state.config.adapter
        )));
    }
    let (manifests, mut references) = manifest_readers(state, &change.changed);
    let (commands, runners) =
        commands::command_runners(&state.config.commands, &state.attestations, &change.changed)?;
    references.extend(runners);
    let mut names = affected_exports(state, network, change)?;
    if !names.is_empty() {
        let index = build_index(&state.attestations, &request_symbols(state)?);
        references.extend(proven(&index, &names));
        references.extend(mentions(state, &index, &names)?);
    }
    names.sort_by(|a, b| compare_utf16(a, b));
    references.sort_by(|a, b| {
        compare_utf16(&a.symbol, &b.symbol).then(compare_utf16(&a.page, &b.page)).then(a.line.cmp(&b.line))
    });
    Ok(Impact { base: label.to_string(), symbols: names, manifests, commands, references })
}

/// The surface exports `change` affects, each once, as the adapter answers in the session
/// this opens.
fn affected_exports(state: &State, network: Network, change: Change) -> Result<Vec<String>> {
    let base: Map<String, Value> =
        change.base.into_iter().map(|(file, text)| (file, text.map_or(Value::Null, Value::String))).collect();
    let opened = open_session(state, network);
    let affected = state.adapter.send("affected", json!({ "base": base, "changed": change.changed }));
    opened.wait()?;
    affected_names(&affected.wait()?)
}

/// The changed manifests that facts read, sorted, and those facts, each a reference to the
/// manifest's call.
fn manifest_readers(state: &State, changed: &[String]) -> (Vec<String>, Vec<ImpactReference>) {
    let readers = state.manifests.readers(&state.attestations, changed);
    let mut files: Vec<String> = readers.iter().map(|(file, _)| file.clone()).collect();
    files.sort_by(|a, b| compare_utf16(a, b));
    files.dedup();
    let references = readers
        .into_iter()
        .map(|(file, fact)| ImpactReference {
            symbol: manifest_call(&file),
            page: fact.page.clone(),
            line: fact.line,
            found: Found::Proven(fact.kind()),
            claim: fact.claim.clone(),
        })
        .collect();
    (files, references)
}

/// The names of the exports `affected` answered, each once, in its order.
fn affected_names(answer: &Value) -> Result<Vec<String>> {
    let refs = answer.as_array().ok_or_else(|| Error::internal("affected did not answer a list"))?;
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for r in refs {
        let name = r.get("name").and_then(Value::as_str);
        let name = name.ok_or_else(|| Error::internal("affected answered an export without a name"))?;
        if seen.insert(name) {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

fn of<'a>(index: &'a Index, symbol: &str) -> &'a [Reference] {
    index.iter().find(|(s, _)| s == symbol).map_or(&[][..], |(_, refs)| refs)
}

fn proven(index: &Index, names: &[String]) -> Vec<ImpactReference> {
    names
        .iter()
        .flat_map(|symbol| {
            of(index, symbol).iter().map(move |r| ImpactReference {
                symbol: symbol.clone(),
                page: r.page.clone(),
                line: r.line,
                found: Found::of(r.kind),
                claim: r.claim.clone(),
            })
        })
        .collect()
}

// Only Markdown pages: a source file is living for its doc-comment examples, and code that
// names a symbol is not prose about it. Which symbol a code span names is the language's to
// say, for every page at once.
fn mentions(state: &State, index: &Index, names: &[String]) -> Result<Vec<ImpactReference>> {
    let pages = pages_naming(state, names)?;
    let mut asked: Vec<&str> = Vec::new();
    let mut seen = HashSet::new();
    for (_, _, spans) in &pages {
        asked.extend(spans.values().into_iter().filter(|v| holds_a_name(v, names) && seen.insert(*v)));
    }
    if asked.is_empty() {
        return Ok(Vec::new());
    }
    let symbols: Vec<&str> = names.iter().map(String::as_str).collect();
    let answers = state.adapter.mentions(&asked, &symbols)?;
    let named: HashMap<&str, &str> = asked
        .iter()
        .zip(&answers)
        .filter_map(|(span, answer)| {
            Some((*span, symbols.iter().copied().find(|s| Some(*s) == answer.as_deref())?))
        })
        .collect();
    let mut out = Vec::new();
    for (page, text, spans) in &pages {
        let on_page: Vec<(&str, Vec<&Reference>)> = names
            .iter()
            .map(|n| (n.as_str(), of(index, n).iter().filter(|r| r.page == **page).collect()))
            .collect();
        let covered = covered::covered_lines(text, &on_page);
        out.extend(mentions::mentions_on(page, spans, &named, &covered));
    }
    Ok(out)
}

fn holds_a_name(text: &str, names: &[String]) -> bool {
    names.iter().any(|n| text.contains(n.as_str()))
}

/// The living Markdown pages that hold a name, with their text and code spans. A name is
/// part of the span that names it, so only pages and spans that hold one count.
fn pages_naming<'s>(
    state: &'s State,
    names: &[String],
) -> Result<Vec<(&'s String, String, mentions::PageSpans)>> {
    let is_page = crate::config::matcher(&state.config.pages)?;
    let mut pages = Vec::new();
    for page in state.living.iter().filter(|p| is_page.is_match(p)) {
        let text = state.files.read(page).map_err(Error::user)?;
        if !holds_a_name(&text, names) {
            continue;
        }
        if let Some(spans) = mentions::code_spans_of(&text) {
            pages.push((page, text, spans));
        }
    }
    Ok(pages)
}
