//! `audit` (including `--apply` and `--inventory`).

use std::path::Path;

use serde_json::Value;

use super::args::Options;
use super::{Io, finish, load, network, pretty};
use crate::api::errors::{Error, Result};
use crate::audit::apply::{ApplyOptions, apply_suggestions, format_apply};
use crate::audit::listing::{format_inventory, run_inventory};
use crate::audit::run::run_audit;
use crate::audit::text::format_audit;
use crate::audit::{InventoryEntry, report_json};

/// `audit`: lists, tables and blocks that look copied from a surface; advisory, so exit 0.
/// With `--inventory [text]`, lists the values a fact could read.
pub(super) fn audit(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    if options.inventory {
        if options.apply {
            return Err(Error::user("--inventory cannot be used with --apply"));
        }
        return inventory(args, root, options, io);
    }
    let state = load(root)?;
    let suggestions = run_audit(&state, network()).map_err(|why| why.within("audit"))?;
    finish(state)?;
    if options.apply {
        let apply_options = ApplyOptions { include_drift: options.include_drift, dry_run: options.dry_run };
        let report = apply_suggestions(root, &suggestions, apply_options)?;
        if options.json {
            io.out(&pretty(&report.to_json(&suggestions)));
        } else {
            io.out(&format_apply(&report));
        }
        return Ok(0);
    }
    if options.json {
        io.out(&pretty(&report_json(&suggestions)));
    } else {
        io.out(&format_audit(&suggestions));
    }
    Ok(0)
}

/// `audit --inventory [text]`: the values a fact could read, each with its expression.
fn inventory(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let state = load(root)?;
    let entries = run_inventory(&state, network(), args.first().map(String::as_str))
        .map_err(|why| why.within("audit"))?;
    finish(state)?;
    if options.json {
        io.out(&pretty(&Value::Array(entries.iter().map(InventoryEntry::to_json).collect())));
    } else {
        io.out(&format_inventory(&entries));
    }
    Ok(0)
}
