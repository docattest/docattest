//! `conformance run`, `conformance record` and `conformance validate`: holding a language
//! adapter to the protocol (PROTOCOL.md §5), for whoever writes or tests one.

use std::path::Path;

use super::Io;
use crate::api::errors::{Error, Result};
use crate::conformance::Adapter;
use crate::conformance::{kit::read_kit, logs::validate_logs, record::record, run_suite};
use crate::files::text_file::read_text;
use crate::schema::Schemas;

const USAGE: &str = "usage: docattest conformance run <kit.json> -- <adapter command...>\n       \
                     docattest conformance record <log> -- <adapter command...>\n       \
                     docattest conformance validate <log>...";

/// Exit code when a check fails or a recorded line does not fit.
const FAILED: i32 = 1;

/// The exit code is `1` when a check does not hold or a recorded line does not fit; `record`
/// exits with the adapter's own code.
pub fn conformance(args: &[String], io: &mut Io) -> Result<i32> {
    let Some((verb, rest)) = args.split_first() else { return Err(Error::user(USAGE)) };
    match (verb.as_str(), rest) {
        ("run", [kit, program, args @ ..]) => run(kit, &Adapter { program, args }, io).map(exit_code),
        ("record", [log, program, args @ ..]) => {
            record(&io.cwd.join(log), &Adapter { program, args }, io.out)
                .map_err(|e| Error::user(format!("conformance record: {e}")))
        }
        ("validate", [_, ..]) => validate(rest, io).map(exit_code),
        _ => Err(Error::user(USAGE)),
    }
}

fn exit_code(held: bool) -> i32 {
    if held { 0 } else { FAILED }
}

/// `run`: every check of the suite against the adapter, with its kit.
fn run(kit: &str, adapter: &Adapter, io: &mut Io) -> Result<bool> {
    let schemas = Schemas::embedded();
    let parsed_kit = read_kit(&read(&io.cwd.join(kit))?, &schemas)
        .map_err(|why| Error::user(format!("{kit} is not a kit the suite can run with:\n  {why}")))?;
    run_suite(adapter, &parsed_kit, &schemas, io.out)
        .map_err(|e| Error::user(format!("conformance run: {e}")))
}

/// `validate`: every recorded line of the logs against the schemas.
fn validate(logs: &[String], io: &mut Io) -> Result<bool> {
    let read_logs =
        logs.iter().map(|log| Ok((log.clone(), read(&io.cwd.join(log))?))).collect::<Result<Vec<_>>>()?;
    validate_logs(&read_logs, io.out).map_err(Error::user)
}

fn read(path: &Path) -> Result<String> {
    read_text(path).map_err(Error::user)
}
