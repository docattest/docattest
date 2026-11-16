//! docattest for a Rust project: the docattest core, told how to start the Rust adapter
//! and what command runs docattest here.

use docattest_rust::launcher::{CORE_VARIABLE, ask, checked, hand_over};
use std::io::Write;

fn main() {
    let args: Vec<String> =
        std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    let command = std::env::current_exe().unwrap_or_else(|_| "docattest".into());
    let added = hand_over(&command, &args, &|name| std::env::var(name).ok());
    let other = std::env::var(CORE_VARIABLE).ok().filter(|core| !core.is_empty());
    if docattest::trace::requested() {
        eprintln!("[trace launcher] core {}", other.as_deref().unwrap_or("(built in)"));
    }
    if let Some(core) = other {
        std::process::exit(run_other(&core, &args, &added));
    }
    for (name, value) in added {
        // SAFETY: no other thread exists yet; the core starts its own after this.
        unsafe { std::env::set_var(name, value) };
    }
    std::process::exit(run_core(&args));
}

/// Runs the core this command was built with, as the core's own command runs it.
fn run_core(args: &[String]) -> i32 {
    docattest::trace::mark("start");
    let cwd = std::env::current_dir().unwrap_or_default();
    // Standard error is not held locked: the adapter reader may trace to it.
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut err = std::io::stderr();
    let code = docattest::cli::run(args, &mut docattest::cli::Io { out: &mut out, err: &mut err, cwd });
    let _ = out.flush();
    drop(out);
    docattest::trace::mark("exit");
    code
}

/// Runs a core someone named in place of the one built in, once it has said what it is.
fn run_other(core: &str, args: &[String], added: &[(&'static str, String)]) -> i32 {
    if let Err(refusal) = checked(core, &ask(core)) {
        eprintln!("{refusal}");
        return 1;
    }
    let ran = std::process::Command::new(core)
        .args(args)
        .envs(added.iter().map(|(name, value)| (*name, value.as_str())))
        .status();
    match ran {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("docattest: cannot run the core ({core}): {err}");
            1
        }
    }
}
