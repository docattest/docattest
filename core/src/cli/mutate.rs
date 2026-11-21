//! `mutate [pages…] [--base <ref>]`: how strong each proof is, by the changes to what it
//! reads that make it fail.

use std::collections::HashSet;
use std::path::Path;

use super::args::Options;
use super::check::page_argument;
use super::{Io, load, network, pretty};
use crate::api::errors::Result;
use crate::mutate::text::format_mutation;
use crate::mutate::{Mutation, changed_attestation_ids, run_mutate_selected};
use crate::project::state::{State, select_pages};

/// The report; exit 1 when a proof survived and `--fail-on-survivors` asks for it, else 0.
pub(super) fn mutate(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let state = load(root)?;
    let selected = selected_pages(args, root, &state)?;
    let mutation = match options.base.as_deref() {
        Some(reference) => mutate_from_base(state, root, reference, &selected)?,
        None => run_mutate_selected(state, network(), &selected, None)?,
    };
    let rendered = if options.json { pretty(&mutation.to_json()) } else { format_mutation(&mutation) };
    io.out(&rendered);
    Ok(mutation.exit_code(options.fail_on_survivors))
}

fn selected_pages(args: &[String], root: &Path, state: &State) -> Result<HashSet<String>> {
    if args.is_empty() {
        return Ok(state.living.iter().cloned().collect());
    }
    let pages: Vec<String> = args.iter().map(|given| page_argument(root, given)).collect();
    select_pages(&pages, &state.living, &state.frozen)
}

fn mutate_from_base(
    state: State,
    root: &Path,
    reference: &str,
    selected: &HashSet<String>,
) -> Result<Mutation> {
    let only = changed_attestation_ids(state, network(), reference, selected)?;
    if only.is_empty() {
        return Ok(Mutation { proofs: Vec::new(), mutants: 0, stillborn: 0 });
    }
    run_mutate_selected(load(root)?, network(), selected, Some(&only))
}
