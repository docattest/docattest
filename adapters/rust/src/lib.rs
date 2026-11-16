//! docattest for Rust projects: the Rust adapter, which the docattest core starts and
//! talks to over the adapter protocol (`protocol/PROTOCOL.md`), and what the `docattest`
//! command of this package needs to hand over to the core.
//!
//! A session ([`session`]) answers what can be read from syntax from syntax (`analysis`,
//! and the questions about text in [`language`]); what has to be compiled or run is
//! given to cargo (`running`, `toolchain`), in a crate the adapter generates beside the
//! project and never inside it.

mod analysis;
#[allow(dead_code, reason = "its users are the generated programs it is copied into")]
mod harness;
pub mod language;
pub mod launcher;
mod project;
pub mod protocol;
mod running;
pub mod session;
mod toolchain;
