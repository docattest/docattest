//! How a block's program says that its own code has started, so that its time limit
//! counts from then and not from its launch: the system's start-up of a freshly linked
//! program is no part of the block's time.

/// The variable that names the file a program creates when its code starts.
pub const VARIABLE: &str = "DOCATTEST_STARTED";

/// Creates the file the adapter named, when it named one. A program that cannot is ended,
/// with why, before any code of its block runs.
pub fn signal() {
    let Some(path) = std::env::var_os(VARIABLE) else { return };
    if let Err(err) = std::fs::write(&path, "") {
        eprintln!("docattest: cannot write {}: {err}", path.to_string_lossy());
        std::process::exit(2);
    }
}
