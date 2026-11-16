//! Console blocks (`src/console/`): transcripts of commands and what they print, which the
//! core runs itself, whatever the project's language. `transcript` reads a block, `matching`
//! compares an output with it, `shell` runs one command and `block` runs a whole block.

pub mod block;
pub mod matching;
pub mod shell;
pub mod transcript;
