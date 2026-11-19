//! What needs no project: the greeting, the questions about Rust text (blocks, doc
//! comments, code spans, fact expressions), and how a data block's values display.

pub(crate) mod blocks;
pub(crate) mod display;
pub(crate) mod doc;
pub(crate) mod text;

pub use doc::RUST_WORDS;
pub use text::{ASSERTING_MACROS, ASSERTING_METHODS};

use crate::protocol::{
    AdapterName, Answer, Capability, DocProse, Example, ExpressionShape, Hello, SourceFiles, SourceText,
    VERSION, Vocabulary, decode,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// The verdict on a block tagged `network` when the network is off.
pub(crate) const NETWORK_SKIPPED: &str =
    "network example skipped: the network is off (set DOCATTEST_NETWORK=1 to run it)";

/// The info-string word by which a block says that panicking is what it proves.
pub(crate) const SHOULD_PANIC: &str = "should_panic";

/// Whether a project path is a Rust source file.
pub(crate) fn is_source(page: &str) -> bool {
    page.ends_with(".rs")
}

/// The greeting: what is fixed for the Rust adapter, whatever the project.
pub fn hello() -> Hello {
    let words = |words: &[&str]| words.iter().map(|word| word.to_string()).collect();
    Hello {
        protocol: VERSION,
        adapter: AdapterName { name: "rust".into(), version: env!("CARGO_PKG_VERSION").into() },
        block_languages: words(&["rust", "rs"]),
        source_files: SourceFiles { include: words(&["**/*.rs"]), exclude: Vec::new() },
        // Where cargo builds, which is never the project's own.
        ignored_names: words(&["target"]),
        capabilities: vec![
            Capability::Tests,
            Capability::RunTests,
            Capability::Observables,
            Capability::Affected,
            Capability::Display,
            Capability::Mutants,
        ],
        info_words: words(&[SHOULD_PANIC]),
        vocabulary: Vocabulary { no_tests: "it declares no test function".into() },
    }
}

#[derive(Deserialize)]
struct Files {
    files: Vec<SourceText>,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

#[derive(Deserialize)]
struct Examples {
    examples: Vec<Code>,
}

#[derive(Deserialize)]
struct Spans {
    spans: Vec<String>,
    symbols: Vec<String>,
}

#[derive(Deserialize)]
struct Expressions {
    expressions: Vec<String>,
}

#[derive(Deserialize)]
struct Values {
    values: Vec<Value>,
}

/// Answers a question about text; `None` for any other method.
pub fn answer(method: &str, params: Value) -> Option<Answer> {
    Some(match method {
        "assertions" => decode::<Examples>(params)
            .map(|p| json!(p.examples.iter().map(|e| text::asserts(&e.code)).collect::<Vec<bool>>())),
        "docComments" => decode::<Files>(params)
            .map(|p| json!(p.files.iter().flat_map(doc::examples).collect::<Vec<Example>>())),
        "docProse" => decode::<Files>(params)
            .map(|p| json!(p.files.iter().map(doc::prose).collect::<Vec<Vec<DocProse>>>())),
        "mentions" => decode::<Spans>(params).map(|p| json!(text::mentions(&p.spans, &p.symbols))),
        "expressions" => decode::<Expressions>(params)
            .map(|p| json!(p.expressions.iter().map(|e| text::shape(e)).collect::<Vec<ExpressionShape>>())),
        "display" => decode::<Values>(params)
            .map(|p| json!(p.values.iter().map(display::shown).collect::<Vec<String>>())),
        _ => return None,
    })
}
