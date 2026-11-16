//! Starting child processes. Every child the core starts, the adapter and git alike, is
//! started by [`spawn`], and by nothing else (`clippy.toml` refuses the direct calls).
//!
//! The rule exists because the core starts children from more than one thread: git reads a
//! base or a change while the adapter starts. A child's pipes are created and then marked
//! close-on-exec, and where the platform cannot do both in one step (macOS has no `pipe2`)
//! a child forked by another thread in between inherits them. An adapter that inherited the
//! write end of git's output pipe keeps it open for as long as it lives, and whoever reads
//! that pipe waits for an end that never comes. One lock around every start closes the gap:
//! while one thread is between creating pipes and returning from the start, no other thread
//! forks. Once a start has returned, every descriptor it left in this process is
//! close-on-exec, so a child started later, at any time and from any thread, inherits none
//! of them: children may live side by side, only their starts are serialised.

use std::process::{Child, Command};
use std::sync::{Mutex, PoisonError};

static SPAWNING: Mutex<()> = Mutex::new(());

/// Start a child process while no other thread starts one.
pub fn spawn(command: &mut Command) -> std::io::Result<Child> {
    let _spawning = SPAWNING.lock().unwrap_or_else(PoisonError::into_inner);
    #[expect(clippy::disallowed_methods, reason = "the one place a child is started, under the lock")]
    command.spawn()
}
