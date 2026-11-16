//! Opt-in timing trace for performance work: with `DOCATTEST_TRACE` set to anything but the
//! empty string, phases and adapter round trips are written to stderr with milliseconds since
//! start. Off by default, so output is untouched.
//!
//! A trace nobody reads is dropped: a write that fails (stderr closed, its reader gone) is
//! ignored, as it is for every other line the core writes to stderr. `eprintln!` would
//! panic instead, on whichever thread traced next; on the adapter's reader thread that left
//! every pending request without an answer.

use std::ffi::OsStr;
use std::io::Write;
use std::sync::OnceLock;
use std::time::Instant;

/// The variable that asks for a trace. Every launcher reads it too, as the core does.
pub const VARIABLE: &str = "DOCATTEST_TRACE";

fn start() -> Option<&'static Instant> {
    static START: OnceLock<Option<Instant>> = OnceLock::new();
    START.get_or_init(|| requested().then(Instant::now)).as_ref()
}

/// Whether this process was asked for a trace.
pub fn requested() -> bool {
    enabled(std::env::var_os(VARIABLE).as_deref())
}

/// Whether the variable asks for a trace: set, and to something. `DOCATTEST_TRACE=` is how a
/// shell spells "off".
fn enabled(value: Option<&OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

/// Record a moment.
pub fn mark(what: &str) {
    if let Some(start) = start() {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let _ = writeln!(std::io::stderr(), "[trace {ms:>7.1} ms] {what}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_variable_set_to_nothing_asks_for_no_trace() {
        assert!(enabled(Some(OsStr::new("1"))) && enabled(Some(OsStr::new("0"))));
        assert!(!enabled(Some(OsStr::new(""))) && !enabled(None));
    }
}
