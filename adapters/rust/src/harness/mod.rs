//! The Rust source that is compiled into the programs docattest generates: the part of
//! a generated program that is the same every time.
//!
//! The modules here are ordinary modules of this crate, built and tested with it. They
//! are also embedded as text, this file among them, and written into the generated crate
//! as its own `harness` module, so each uses only the standard library and names its
//! siblings through `super`. A generated program must build wherever the project builds,
//! with no crate the project did not ask for.

pub mod json;
pub mod observe;
pub mod serve;
