//! `audit` and `inventory`.

use std::path::Path;

use serde_json::Value;

use super::args::Options;
use super::{Io, finish, load, network, pretty};
use crate::api::errors::Result;
use crate::audit::listing::{format_inventory, run_inventory};
use crate::audit::run::run_audit;
use crate::audit::text::format_audit;
use crate::audit::{InventoryEntry, report_json};

/// `audit`: lists, tables and blocks that look copied from a surface; advisory, so exit 0.
pub(super) fn audit(root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let state = load(root)?;
    let suggestions = run_audit(&state, network()).map_err(|why| why.within("audit"))?;
    finish(state)?;
    if options.json {
        io.out(&pretty(&report_json(&suggestions)));
    } else {
        io.out(&format_audit(&suggestions));
    }
    Ok(0)
}

/// `inventory [text]`: the values a fact could read, each with its expression.
pub(super) fn inventory(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let state = load(root)?;
    let entries = run_inventory(&state, network(), args.first().map(String::as_str))
        .map_err(|why| why.within("inventory"))?;
    finish(state)?;
    if options.json {
        io.out(&pretty(&Value::Array(entries.iter().map(InventoryEntry::to_json).collect())));
    } else {
        io.out(&format_inventory(&entries));
    }
    Ok(0)
}
