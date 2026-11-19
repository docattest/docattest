//! `mutate [pages…]`: how strong each proof is, by the changes to what it reads that make it
//! fail.

use std::path::Path;

use super::args::Options;
use super::check::page_argument;
use super::{Io, load, network, pretty};
use crate::api::errors::Result;
use crate::mutate::run_mutate;
use crate::mutate::text::format_mutation;
use crate::project::state::select_pages;

/// The report; exit 1 when a proof survived and `--fail-on-survivors` asks for it, else 0.
pub(super) fn mutate(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    let pages: Vec<String> = args.iter().map(|given| page_argument(root, given)).collect();
    let state = load(root)?;
    let selected = if pages.is_empty() {
        state.living.iter().cloned().collect()
    } else {
        select_pages(&pages, &state.living, &state.frozen)?
    };
    let mutation = run_mutate(state, network(), &selected)?;
    io.out(&if options.json { pretty(&mutation.to_json()) } else { format_mutation(&mutation) });
    Ok(mutation.exit_code(options.fail_on_survivors))
}
