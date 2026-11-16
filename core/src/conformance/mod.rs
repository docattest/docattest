//! The conformance suite of the adapter protocol (PROTOCOL.md §5), for an adapter in any
//! language. It starts the adapter as the core does and holds it to the protocol: the order
//! of things, the errors, the shape of every answer (against the schemas in [`crate::schema`]),
//! and that each answer is there. It knows no language: what it must be told about one is the
//! adapter's kit ([`kit`]).
//!
//! Beside it are a recorder that stands between a core and an adapter ([`record`]) and a
//! check of what it recorded against the schemas ([`logs`]).

mod checks;
mod expect;
pub mod kit;
pub mod logs;
pub mod record;
mod requests;
mod suite;
mod talk;

use std::io::Write;

use crate::counted::{agreeing, counted};
use crate::schema::Schemas;
use kit::Kit;
use suite::Suite;

/// The command that starts the adapter: its program and arguments.
pub struct Adapter<'a> {
    pub program: &'a str,
    pub args: &'a [String],
}

/// How many lines of a failure's message are printed under the check's name.
const MESSAGE_LINES: usize = 12;

/// Runs every check against `adapter` with `kit`, printing one line for each and then the
/// count. Whether every check held.
pub fn run_suite(
    adapter: &Adapter,
    kit: &Kit,
    schemas: &Schemas,
    out: &mut dyn Write,
) -> std::io::Result<bool> {
    let suite = Suite::new(kit, adapter, schemas);
    let checks = checks::checks(kit);
    let mut failed = 0;
    for check in &checks {
        let outcome = (check.run)(&suite).and_then(|talked| {
            expect::ensure(
                talked.drift.is_empty(),
                format!("every line fits the schemas: {}", talked.drift.join("\n")),
            )
        });
        let line = match outcome {
            Ok(()) => format!("✓ {}", check.name),
            Err(message) => {
                failed += 1;
                let shown: Vec<&str> = message.split('\n').take(MESSAGE_LINES).collect();
                format!("✗ {}\n    {}", check.name, shown.join("\n    "))
            }
        };
        writeln!(out, "{line}")?;
        out.flush()?;
    }
    let held = checks.len() - failed;
    let verb = agreeing(checks.len(), "holds", "hold");
    writeln!(out, "conformance ({}): {held} of {} {verb}", kit.adapter, counted(checks.len(), "check"))?;
    Ok(failed == 0)
}
