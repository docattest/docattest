//! Version 1 of the docattest adapter protocol as this adapter speaks it: the data types
//! of the schemas in `protocol/schema/` (`types`) and the loop that reads requests and
//! writes responses (`server`).

mod server;
mod types;

pub use server::{Adapter, Answer, Error, Session, decode, serve};
pub use types::*;

/// The protocol number this adapter speaks: the core's own, since the adapter is built
/// with the core it runs with.
pub const VERSION: u64 = docattest::adapter::PROTOCOL;
