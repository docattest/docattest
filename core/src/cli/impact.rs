//! `impact [--base <ref>] [--file <path>]`: the exports a diff affects and the prose that
//! names them.

use std::path::Path;

use super::args::Options;
use super::hook::{Edited, impact_of_edit, locate};
use super::{Io, finish, network, pretty, resolve};
use crate::adapter::Start;
use crate::api::errors::{Error, Result};
use crate::files::source::DiskFiles;
use crate::git::change::git_change;
use crate::impact::text::format_impact;
use crate::impact::{Impact, Scope, read_change, run_impact};
use crate::project::state::{Greeted, start};
use crate::snapshots::now_ms;

/// The impact of the working tree's difference from `--base` on the project at `root`.
fn impact_of(root: &Path, options: &Options) -> Result<Impact> {
    // A project without a valid config fails before git is asked anything, so the config
    // is the error reported, not whatever git would say.
    impact_on(start(DiskFiles::new(root.to_path_buf()), Start::Warm)?.greet()?, options, &Scope::Diff)
}

/// The same for a project that was started, within `scope`. Git reads the change while the
/// pages are parsed and the adapter answers; what it found (or why it failed) is looked at
/// only once the project has loaded.
pub(super) fn impact_on(greeted: Greeted, options: &Options, scope: &Scope) -> Result<Impact> {
    let label = options.base.as_deref().unwrap_or("HEAD");
    let root = greeted.files.root().to_path_buf();
    let (state, change) = std::thread::scope(|threads| {
        let change = threads.spawn(|| read_change(&root, label, scope));
        let state = greeted.load(now_ms());
        (state, change.join())
    });
    let state = state?;
    let change = change.unwrap_or_else(|_| Err(Error::internal("reading the change from git failed")));
    let found = run_impact(&state, network(), label, change)?;
    finish(state)?;
    Ok(found)
}

/// `impact [--base <ref>] [--json]`, and `impact --hook`. With `--file <path>` it is what any
/// agent or script asks after editing that file: the same answer for that file's change alone
/// when it is a source file of the project and affects an export, and nothing at all otherwise.
pub(super) fn impact(args: &[String], root: &Path, options: &Options, io: &mut Io) -> Result<i32> {
    if !args.is_empty() {
        return Err(Error::user("usage: docattest impact [--base <ref>] [--file <path>] [--json]"));
    }
    if options.hook {
        let event = super::hook::read_stdin();
        return super::hook::impact_hook(&event, root, options, io);
    }
    if let Some(file) = &options.file {
        if let Some(found) = impact_of_file(file, root, options, io)? {
            io.out(&if options.json { pretty(&found.to_json()) } else { format_impact(&found) });
        }
        return Ok(0);
    }
    let found = impact_of(root, options)?;
    io.out(&if options.json { pretty(&found.to_json()) } else { format_impact(&found) });
    Ok(0)
}

/// `impact --file <given>`. The path is read from the working directory, as a shell gave it.
/// Whoever names a file means one: a path outside the project, or one that is neither on
/// disk nor in the diff, is their mistake, never a quiet "nothing affected", because a
/// path written from somewhere else would otherwise pass for a file that changed nothing.
fn impact_of_file(given: &str, root: &Path, options: &Options, io: &Io) -> Result<Option<Impact>> {
    if given.is_empty() {
        return Err(Error::user("impact --file needs the path of the file that was edited"));
    }
    let file = resolve(&io.cwd, given);
    let (root, rel) = match locate(&file, root, options) {
        Edited::Inside { root, rel } => (root, rel),
        Edited::Outside { root } => {
            return Err(Error::user(format!(
                "impact --file {given}: the path is outside the project at {} \
                 (a relative path is read from the working directory)",
                root.display()
            )));
        }
    };
    if !file.exists() {
        let label = options.base.as_deref().unwrap_or("HEAD");
        if !git_change(&root, label)?.changed.contains(&rel) {
            return Err(Error::user(format!(
                "impact --file {given}: there is no file {}, and the change since {label} does not touch it \
                 (a relative path is read from the working directory)",
                file.display()
            )));
        }
    }
    impact_of_edit(&root, &rel, options)
}
