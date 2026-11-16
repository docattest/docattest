//! The docattest command line.

use std::io::Write;

fn main() {
    docattest::trace::mark("start");
    let argv: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    // Standard error is not held locked: the adapter reader may trace to it.
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut err = std::io::stderr();
    let code = docattest::cli::run(&argv, &mut docattest::cli::Io { out: &mut out, err: &mut err, cwd });
    let _ = out.flush();
    drop(out);
    docattest::trace::mark("exit");
    std::process::exit(code);
}
