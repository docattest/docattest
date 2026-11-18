//! The requests the checks send: those every adapter is sent alike, and those built from the
//! kit ([`Requests`]).

use std::cell::Cell;

use serde_json::{Value, json};

use super::kit::{Kit, KitExample};

/// How far apart the lines of the suite's attestations are on the page.
const LINE_STEP: u64 = 10;

/// The one manifest every session of the suite opens with, which the kit's manifest fact
/// reads: its path, and the value of its field `version`.
pub const MANIFEST: &str = "meta.json";
pub const MANIFEST_VERSION: &str = "1.2.0";

/// The one file every session of the suite opens with as text, which a fact reads with
/// `file`: its path, and its text.
pub const TEXT_FILE: &str = "Dockerfile";
pub const TEXT: &str = "FROM scratch\nUSER 10001\n";

/// The greeting every conversation starts with.
pub fn hello() -> Value {
    request(1, "hello", json!({ "protocol": crate::adapter::PROTOCOL }))
}

/// A request, as the core sends it.
pub fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "id": id, "method": method, "params": params })
}

/// The requests built from the kit. Each attestation they make is at a line of its own: the
/// lines count up across every check of a run, in the order the checks make them.
pub struct Requests<'a> {
    kit: &'a Kit,
    line: Cell<u64>,
}

impl<'a> Requests<'a> {
    pub fn new(kit: &'a Kit) -> Requests<'a> {
        Requests { kit, line: Cell::new(1) }
    }

    /// The page the attestations claim to be on.
    pub fn page(&self) -> &str {
        &self.kit.page
    }

    fn next_line(&self) -> u64 {
        self.line.set(self.line.get() + LINE_STEP);
        self.line.get()
    }

    /// An example attestation of a piece of the kit's code, to be checked.
    pub fn example(&self, id: &str, example: &KitExample) -> Value {
        self.example_in_mode(id, example, "check")
    }

    /// An example attestation of a piece of the kit's code, in a mode of the protocol's.
    pub fn example_in_mode(&self, id: &str, example: &KitExample, mode: &str) -> Value {
        let mut attestation = json!({
            "id": id, "page": self.page(), "line": self.next_line(), "claim": "", "kind": "example",
            "lang": example.lang, "mode": mode, "network": false, "code": example.code, "weak": false,
        });
        if let Some(tags) = &example.tags {
            attestation["tags"] = json!(tags);
        }
        attestation
    }

    /// A fact attestation of an expression on the default surface.
    pub fn fact(&self, id: &str, expression: &str) -> Value {
        json!({
            "id": id, "page": self.page(), "line": self.next_line(), "claim": "", "kind": "fact",
            "expression": expression, "surface": "default", "ordered": false,
            "target": { "kind": "scalar", "value": "" },
        })
    }

    /// `open` of the kit's project, as request 2, with `examples` to compile.
    pub fn open(&self, examples: Value) -> Value {
        self.open_as(2, examples)
    }

    /// `open` of the kit's project, as request `id`, with `examples` to compile.
    pub fn open_as(&self, id: u64, examples: Value) -> Value {
        let mut params = json!({
            "root": "/conformance/not/read",
            "files": self.kit.files,
            "surfaces": self.kit.surfaces,
            "timeoutMs": 20_000,
            "workers": 2,
            "network": false,
            "snapshots": { "data": {} },
            "manifests": { MANIFEST: { "version": MANIFEST_VERSION } },
            "texts": { TEXT_FILE: TEXT },
            "examples": examples,
        });
        if let Some(setup) = &self.kit.setup {
            params["setup"] = json!(setup);
        }
        request(id, "open", params)
    }

    /// hello, open with `examples`, then each `(method, params)` with ids from 3; the input
    /// ends after them.
    pub fn session(&self, requests: &[(&str, Value)], examples: Value) -> Vec<Value> {
        let numbered =
            requests.iter().zip(3..).map(|((method, params), id)| request(id, method, params.clone()));
        [hello(), self.open(examples)].into_iter().chain(numbered).collect()
    }
}
