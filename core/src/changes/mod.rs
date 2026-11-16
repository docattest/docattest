//! Changes against a base: what the working tree did to the attestations that existed at a
//! git ref (`src/changes/`, `check --base <ref>`).
//!
//! A verdict depends only on the current page and code; a weakening is a property of a
//! change. The base is read from git into memory and planned without running anything, its
//! attestations are matched to the head's by what they are and what they defend, and what
//! is left is classified: `orphaned-claim` (fail), `frozen-page`, `downgrade` and `weakened`
//! (warnings).

pub mod change;
pub mod compare;
pub mod frozen;
pub mod judge;
pub mod matching;
pub mod orphans;
pub mod project;
pub mod side;
pub mod signature;
pub mod text;
pub mod units;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use crate::api::errors::{Error, Result};
use crate::config::CONFIG_FILE;
use crate::files::ignored::Ignored;
use crate::files::memory::MemoryFiles;
use crate::git;
use crate::project::state::State;
use crate::report::Report;
use change::Severity::Fail;
pub use change::{Change, change_lines};
use project::{Planned, head_of, plan_memory};

/// The base, planned: nothing when it had no docattest config, so nothing to protect.
pub struct Baseline {
    planned: Option<Planned>,
}

/// The base being read from git, on another thread.
pub struct BaseReading {
    reference: String,
    /// Whether the base was found: sent once, before its files are read.
    found: Receiver<Result<()>>,
    /// Where the reading thread waits for the names the language ignores.
    ignored: Sender<Ignored>,
    files: JoinHandle<Result<Vec<(String, String)>>>,
}

/// Start reading the base at `reference` on another thread, which overlaps with loading the
/// project and with the check. The thread first finds the base ([`git::base::find`]: the
/// repository, the ref, the files committed there and the config among them), says so, and
/// then reads the files planning needs, once it has been told which names the language
/// ignores ([`BaseReading::ignore`]). Nothing it finds is looked at here: a missing
/// repository, an unknown ref or an invalid base config is reported by
/// [`BaseReading::found`], and the files by [`load_base`].
///
/// Git may run at any time, also while the adapter is being started: every child is started
/// under the spawn lock (`process`), so none inherits the pipes of another.
pub fn read_base(root: &Path, reference: &str) -> BaseReading {
    let (root, wanted) = (root.to_path_buf(), reference.to_string());
    let (found, finding) = channel();
    let (ignored, ignoring) = channel::<Ignored>();
    let files = std::thread::spawn(move || {
        let base = git::base::find(&root, &wanted);
        // The receiver may be gone: the project failed to load, and nobody asks any more.
        let _ = found.send(base.as_ref().map(|_| ()).map_err(Clone::clone));
        let base = base?;
        // Which files are the project's own is the adapter's to say, in its greeting.
        let names = ignoring.recv().map_err(|_| Error::internal("the base was never asked for"))?;
        base.read(&names)
    });
    BaseReading { reference: reference.to_string(), found: finding, ignored, files }
}

impl BaseReading {
    /// Say what the language never counts as the project's own (its `ignoredNames`);
    /// the base's files are read from here on.
    pub fn ignore(&self, ignored: &Ignored) {
        // The thread is gone only if finding the base failed, which `found` reports.
        let _ = self.ignored.send(ignored.clone());
    }

    /// Wait until the base is found. What a user can have got wrong about a base fails
    /// here, so that it fails before anything runs: no git, no repository, no such commit,
    /// a config at the base that is not valid.
    pub fn found(&self) -> Result<()> {
        self.found.recv().unwrap_or_else(|_| Err(Error::internal("reading the base from git failed")))
    }
}

/// Plan the base read from git, parse-only. The base is read with the head's adapter, given
/// the base's file contents, whatever adapter the base's own config named.
pub fn load_base(state: &State, reading: BaseReading) -> Result<Baseline> {
    let reference = reading.reference.as_str();
    let files = reading.files.join().map_err(|_| Error::internal("reading the base from git failed"))??;
    crate::trace::mark("base read from git");
    if !files.iter().any(|(path, _)| path == CONFIG_FILE) {
        return Ok(Baseline { planned: None });
    }
    let planned = plan_memory(MemoryFiles::new(files), &state.adapter);
    crate::trace::mark("base planned");
    match planned {
        Ok(planned) => Ok(Baseline { planned: Some(planned) }),
        Err(error @ Error::User(_)) => {
            Err(Error::user(format!("the base \"{reference}\": {}", git::base::unprefixed(&error))))
        }
        Err(other) => Err(other),
    }
}

/// Add the changes since the base to the report; a `fail` change fails the check. The head's
/// session must still be open: the adapter may be asked which names its code still holds.
pub fn with_changes(report: &mut Report, baseline: &Baseline, head: &State) -> Result<()> {
    let now = head_of(head);
    let base = baseline.planned.as_ref().map(|planned| planned.project(&head.adapter));
    let changes = compare::changes_between(base.as_ref(), &now)?;
    crate::trace::mark("changes compared");
    report_changes(report, changes);
    Ok(())
}

/// Put the changes in the report; a `fail` change fails it, a warning never does.
pub fn report_changes(report: &mut Report, changes: Vec<Change>) {
    if changes.iter().any(|c| c.severity == Fail) {
        report.exit_code = 1;
    }
    report.changes = Some(changes);
}
