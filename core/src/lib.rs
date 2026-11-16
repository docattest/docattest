//! The language-neutral core of docattest: configuration, pages, directives, binding,
//! comparison, the index, the report and exit codes. A language adapter runs as a child
//! process and answers through the protocol in `protocol/PROTOCOL.md` (see [`adapter`]).
//!
//! What the core prints is held to the parity recordings (`tests/parity/recordings/` in the
//! repository), byte for byte. Where a comment says a rule is kept "as the recordings hold",
//! the rule decides output those recordings fix, and changing it is a change of behaviour:
//! record it on purpose, as `tests/README.md` describes.

// The library's public API is what its README, the two binaries, the integration tests and
// the development tools in `examples/` use. Every other module is private to the crate, so
// the compiler reports what nothing calls.
pub mod adapter;
pub mod agents;
mod api;
mod attestations;
mod audit;
mod changes;
pub mod cli;
mod compare;
pub mod config;
mod conformance;
mod console;
mod counted;
mod files;
mod git;
mod impact;
mod json;
mod manifests;
mod markdown;
mod parallel;
pub mod process;
mod project;
mod report;
mod schema;
mod snapshots;
pub mod text;
pub mod trace;

/// What the development tools in `examples/` read: the page model, the plan of a page and
/// the error they report.
pub mod debug {
    pub use crate::api::errors::Error;
    pub use crate::attestations::{examples::ExampleOptions, plan::plan_page};
    pub use crate::markdown::{model, parse::parse_page};
}
