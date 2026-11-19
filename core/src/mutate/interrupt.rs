//! What `mutate` removes if it is stopped by a signal: the copies of the project it made
//! beside it (and their locks). A copy is removed when it is dropped, which covers an
//! ordinary end and a panic; a signal (Ctrl-C, SIGTERM) ends the process without dropping
//! anything, so the copies alive at that moment are held here, and a thread woken by the
//! signal removes them before the process exits as the signal would have ended it. A run
//! killed outright (SIGKILL, a power cut) leaves its copies behind; their locks are free
//! then, and the next run removes them ([`super::sibling::clear_stale`]).
//!
//! The paths are held in a process-wide list because a signal reaches the process, not a
//! run: there is one list however many copies are made.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};

/// The paths removed if a signal stops the process, each copy's directory and its lock.
static HELD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Remove `paths` if a signal stops the process before [`release`] is called for them.
pub fn hold(paths: &[&Path]) {
    static WATCHING: Once = Once::new();
    WATCHING.call_once(watch);
    if let Ok(mut held) = HELD.lock() {
        held.extend(paths.iter().map(|p| p.to_path_buf()));
    }
}

/// No longer remove `paths` on a signal: they were removed, or are being.
pub fn release(paths: &[&Path]) {
    if let Ok(mut held) = HELD.lock() {
        held.retain(|held| !paths.contains(&held.as_path()));
    }
}

/// Remove every path held. A directory is removed with what it holds, and a link in it is
/// removed, never followed.
fn remove_held() {
    let held = HELD.lock().map(|held| held.clone()).unwrap_or_default();
    for path in held {
        let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    }
}

#[cfg(unix)]
use unix::watch;

#[cfg(not(unix))]
fn watch() {
    // Without a console handler, an interrupted run's copies are removed by the next run.
}

#[cfg(unix)]
mod unix {
    //! The self-pipe: the signal handler only writes the signal's number to a pipe, which is
    //! all a handler may safely do; a thread reading the pipe does the removing.

    use std::io::Read;
    use std::os::fd::IntoRawFd;
    use std::os::raw::{c_int, c_void};
    use std::sync::atomic::{AtomicI32, Ordering};

    const SIGINT: c_int = 2;
    const SIGTERM: c_int = 15;

    unsafe extern "C" {
        fn signal(signum: c_int, handler: usize) -> usize;
        fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    }

    /// The pipe's end the handler writes to; -1 before it is made.
    static WAKE: AtomicI32 = AtomicI32::new(-1);

    extern "C" fn on_signal(number: c_int) {
        let byte = number as u8;
        // SAFETY: write(2) is async-signal-safe, and the buffer is one byte on the stack.
        unsafe { write(WAKE.load(Ordering::SeqCst), (&raw const byte).cast(), 1) };
    }

    /// Start the thread that removes the copies when SIGINT or SIGTERM arrives, and make
    /// those signals wake it.
    pub fn watch() {
        let Ok((mut reader, writer)) = std::io::pipe() else { return };
        WAKE.store(writer.into_raw_fd(), Ordering::SeqCst);
        std::thread::spawn(move || {
            let mut number = [0u8];
            if reader.read_exact(&mut number).is_ok() {
                super::remove_held();
                std::process::exit(128 + i32::from(number[0]));
            }
        });
        let handler = on_signal as extern "C" fn(c_int) as usize;
        // SAFETY: the handler only calls write(2) on a descriptor that stays open.
        unsafe {
            signal(SIGINT, handler);
            signal(SIGTERM, handler);
        }
    }
}
