//! `refresh [name]`: refetch snapshots and rewrite their files.

use std::path::Path;

use super::Io;
use crate::adapter::Start;
use crate::api::errors::Result;
use crate::files::source::DiskFiles;
use crate::project::state::start;
use crate::snapshots::now_ms;
use crate::snapshots::refresh::refresh_snapshots;

/// `refresh [name]`.
pub(super) fn refresh(args: &[String], root: &Path, io: &mut Io) -> Result<i32> {
    // No session follows, so the adapter starts cold: it loads its compiler only if the
    // project has doc comments for it to read.
    let state = start(DiskFiles::new(root.to_path_buf()), Start::Cold)?.load(now_ms())?;
    let written = refresh_snapshots(&state, args.first().map(String::as_str), now_ms())?;
    // No session was opened.
    state.adapter.finish()?;
    for file in &written {
        io.out(&format!("wrote {file}"));
    }
    if written.is_empty() {
        io.out("no snapshots to refresh");
    }
    Ok(0)
}
