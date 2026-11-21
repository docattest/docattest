//! Running a console block: each command in turn, until one does not do what the block
//! shows.

use std::path::Path;
use std::time::Duration;

use super::matching::first_mismatch;
use super::shell::{Ran, Shell, shell_word};
use super::transcript::{Step, drop_trailing_blanks};
use crate::api::attestation::{Attestation, Console, Kind};
use crate::report::{Outcome, Status};
use crate::text::{is_js_space, lines};

/// The console blocks whose commands are running, one `<page path>:<line>` a line. A
/// command that runs docattest on the page it is on finds its own block here, and does not
/// run it again for ever.
const RUNNING_VARIABLE: &str = "DOCATTEST_CONSOLE_RUNNING";

/// Where a project's console blocks run: its root, how long a command may take, and the
/// variables that start its adapters where a run inside one would not find them.
pub struct ConsolePlace<'a> {
    pub root: &'a Path,
    pub timeout_ms: u64,
    pub adapters: Vec<(String, String)>,
}

/// Run the block's commands, in order; the first that does not print or exit as the block
/// shows fails it, and the rest do not run.
pub fn run_console(attestation: &Attestation, console: &Console, place: &ConsolePlace) -> Outcome {
    run_console_captured(attestation, console, place).0
}

/// Like [`run_console`], also returning the normalized lines printed by each step that
/// ran and passed (for `mutate` to test what the block's transcript actually checks).
pub fn run_console_captured(
    attestation: &Attestation,
    console: &Console,
    place: &ConsolePlace,
) -> (Outcome, Vec<Vec<String>>) {
    let verdict = |status: Status, message: &str| {
        Outcome::new(&attestation.page, attestation.line, Kind::Example, status, message)
    };
    let stop = |status: Status, message: &str| (verdict(status, message), Vec::new());
    if cfg!(windows) {
        return stop(Status::Skipped, "console blocks run with sh, which docattest does not use on Windows");
    }
    let key = format!("{}:{}", place.root.join(&attestation.page).display(), attestation.line);
    let Some(chain) = running_with(key) else {
        return stop(Status::Error, "this block is already running: a command in it checks this page again");
    };
    let written = console.cwd.as_deref().unwrap_or(".");
    let cwd = place.root.join(written);
    if !cwd.is_dir() {
        return stop(Status::Error, &format!("cwd={written} is not a directory of the project"));
    }
    let prelude = match docattest_function() {
        Ok(prelude) => prelude,
        Err(e) => return stop(Status::Error, &format!("cannot tell where docattest is: {e}")),
    };
    let adapter = place.adapters.iter().map(|(name, command)| (name.as_str(), command.clone()));
    let env: Vec<(&str, String)> = [(RUNNING_VARIABLE, chain)].into_iter().chain(adapter).collect();
    let shell = Shell { cwd: &cwd, timeout: Duration::from_millis(place.timeout_ms), env: &env };
    let mut captured = Vec::with_capacity(console.steps.len());
    for step in &console.steps {
        let ran = match shell.run(&format!("{prelude}{}", step.command)) {
            Ok(ran) => ran,
            Err(e) => return stop(Status::Error, &format!("{} cannot be run: {e}", at(step))),
        };
        if let Some(failure) = judge(step, &ran, place.timeout_ms) {
            let failed = verdict(Status::Fail, &failure.message);
            let outcome = match failure.comparison {
                Some((expected, actual)) => failed.with_comparison(&expected, &actual),
                None => failed,
            };
            return (outcome, Vec::new());
        }
        captured.push(printed_lines(&ran));
    }
    (verdict(Status::Pass, "every command printed what the block shows"), captured)
}

fn printed_lines(ran: &Ran) -> Vec<String> {
    let Ran::Exited { stdout, stderr, .. } = ran else { return Vec::new() };
    let mut printed = output_lines(stdout);
    printed.extend(output_lines(stderr));
    drop_trailing_blanks(&mut printed);
    printed
}

/// The blocks whose commands are running, this one added; None when it is among them already.
fn running_with(key: String) -> Option<String> {
    let running = std::env::var(RUNNING_VARIABLE).unwrap_or_default();
    if running.lines().any(|line| line == key) {
        return None;
    }
    Some(if running.is_empty() { key } else { format!("{running}\n{key}") })
}

/// The shell function `docattest`, which runs this very executable: the core that checks
/// the page, with the environment (and so the adapters) it was started with.
fn docattest_function() -> std::io::Result<String> {
    let docattest = std::env::current_exe()?;
    Ok(format!("docattest() {{ {} \"$@\"; }}\n", shell_word(&docattest.to_string_lossy())))
}

/// Why a command fails its block, with the output the block shows and the one it printed
/// when they differ.
#[derive(Debug, PartialEq, Eq)]
struct Failure {
    message: String,
    comparison: Option<(String, String)>,
}

/// Whether a command did what the block shows: it finished in time, printed the block's
/// output, then exited with the block's code.
fn judge(step: &Step, ran: &Ran, timeout_ms: u64) -> Option<Failure> {
    let fails = |why: String| Some(Failure { message: format!("{}: {why}", at(step)), comparison: None });
    let Ran::Exited { code, stdout, stderr } = ran else {
        return fails(format!("timed out after {timeout_ms} ms"));
    };
    let mut printed = output_lines(stdout);
    printed.extend(output_lines(stderr));
    drop_trailing_blanks(&mut printed);
    if let Some(mismatch) = first_mismatch(&step.output, &printed) {
        let comparison = Some((step.output.join("\n"), printed.join("\n")));
        return Some(Failure { message: format!("{}: {}", at(step), mismatch.describe()), comparison });
    }
    match code {
        Some(code) if *code == step.exit => None,
        Some(code) if step.exit == 0 => {
            fails(format!("exited with {code}; end its output in the block with [exit {code}] if it should"))
        }
        Some(code) => fails(format!("exited with {code}, not {}", step.exit)),
        None => fails("was ended by a signal".to_string()),
    }
}

/// A command as a failure names it: the command, and its line in the block.
fn at(step: &Step) -> String {
    format!("`{}` (block line {})", step.command, step.line)
}

/// The lines of one output, without trailing whitespace. The newline that ends the output
/// does not begin another line.
fn output_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_suffix('\n').unwrap_or(&text);
    if text.is_empty() {
        return Vec::new();
    }
    lines(text).into_iter().map(|line| line.trim_end_matches(is_js_space).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(output: &[&str], exit: i32) -> Step {
        Step { line: 2, command: "tool".into(), output: output.iter().map(|l| l.to_string()).collect(), exit }
    }

    fn exited(code: i32, stdout: &str, stderr: &str) -> Ran {
        Ran::Exited { code: Some(code), stdout: stdout.into(), stderr: stderr.into() }
    }

    fn message(step: &Step, ran: &Ran) -> Option<String> {
        judge(step, ran, 50).map(|f| f.message)
    }

    #[test]
    fn stdout_then_stderr_is_the_output() {
        assert_eq!(judge(&step(&["a", "", "b"], 0), &exited(0, "a\n\n", "b  \r\n\n"), 50), None);
        assert_eq!(judge(&step(&["a"], 0), &exited(0, "a", ""), 50), None);
        assert_eq!(judge(&step(&[], 0), &exited(0, "", "\n"), 50), None);
        let failure = judge(&step(&["b", "a"], 0), &exited(0, "a\n", "b\n"), 50).unwrap();
        assert_eq!(
            failure.message,
            "`tool` (block line 2): output line 1 differs: expected \"b\", got \"a\""
        );
        assert_eq!(failure.comparison, Some(("b\na".into(), "a\nb".into())));
    }

    #[test]
    fn the_exit_code_must_be_the_blocks() {
        assert_eq!(
            message(&step(&[], 0), &exited(1, "", "")).unwrap(),
            "`tool` (block line 2): exited with 1; end its output in the block with [exit 1] if it should"
        );
        assert_eq!(message(&step(&[], 1), &exited(1, "", "")), None);
        assert_eq!(
            message(&step(&[], 1), &exited(0, "", "")).unwrap(),
            "`tool` (block line 2): exited with 0, not 1"
        );
        let signalled = Ran::Exited { code: None, stdout: Vec::new(), stderr: Vec::new() };
        assert_eq!(
            message(&step(&[], 0), &signalled).unwrap(),
            "`tool` (block line 2): was ended by a signal"
        );
    }

    #[test]
    fn the_output_is_judged_before_the_exit_code() {
        assert_eq!(
            message(&step(&["x"], 0), &exited(2, "y\n", "")).unwrap(),
            "`tool` (block line 2): output line 1 differs: expected \"x\", got \"y\""
        );
        assert_eq!(
            message(&step(&[], 0), &Ran::TimedOut).unwrap(),
            "`tool` (block line 2): timed out after 50 ms"
        );
    }
}
