//! Running programs for the adapter: cargo ([`cargo`]), rustdoc ([`rustdoc`]), and the
//! programs it builds from attested code.

pub mod cargo;
pub mod rustdoc;

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

/// What is kept of each stream of a program.
const MAX_OUTPUT: u64 = 1 << 20;

/// A program to run.
#[derive(Default)]
pub struct Command {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The working directory.
    pub dir: PathBuf,
    /// Ends the program, and everything it started, when it is given.
    pub timeout: Option<Duration>,
    /// Variables set for the program, over the adapter's own environment.
    pub env: Vec<(String, String)>,
}

/// How a program ended.
#[derive(Debug, Default)]
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    /// The exit status; `None` when the program did not exit by itself.
    pub code: Option<i32>,
    pub timed_out: bool,
    /// Why the program could not be run at all.
    pub error: Option<String>,
}

/// Runs a program to its end. Its output is kept, and never reaches the adapter's own
/// standard output, which carries only responses.
pub fn run(command: &Command) -> Outcome {
    let mut process = Process::new(&command.program);
    // The output is read as text, never shown on a terminal: cargo must not colour it, even
    // where the environment asks it to (CI often sets CARGO_TERM_COLOR=always).
    process.env("CARGO_TERM_COLOR", "never");
    process.args(&command.args).current_dir(&command.dir).envs(command.env.iter().cloned());
    process.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    own_group(&mut process);
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(err) => return Outcome { error: Some(err.to_string()), ..Outcome::default() },
    };
    let streams = (reader(child.stdout.take()), reader(child.stderr.take()));
    let (code, timed_out) = wait(&mut child, command.timeout);
    let read = |stream: std::thread::JoinHandle<String>| stream.join().unwrap_or_default();
    Outcome { stdout: read(streams.0), stderr: read(streams.1), code, timed_out, error: None }
}

/// Reads a stream on a thread of its own, keeping its first [`MAX_OUTPUT`] bytes.
fn reader(stream: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.by_ref().take(MAX_OUTPUT).read_to_end(&mut kept);
            let _ = std::io::copy(&mut stream, &mut std::io::sink());
        }
        String::from_utf8_lossy(&kept).into_owned()
    })
}

/// Waits for a program to exit, or ends it when its time is up.
fn wait(child: &mut Child, timeout: Option<Duration>) -> (Option<i32>, bool) {
    let deadline = timeout.map(|limit| Instant::now() + limit);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (status.code(), false),
            Ok(None) if deadline.is_some_and(|at| Instant::now() >= at) => {
                end(child);
                let _ = child.wait();
                return (None, true);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => return (None, false),
        }
    }
}

/// Gives a program a process group of its own, so that ending it ends what it started.
#[cfg(unix)]
fn own_group(process: &mut Process) {
    use std::os::unix::process::CommandExt;
    process.process_group(0);
}

#[cfg(not(unix))]
fn own_group(_: &mut Process) {}

/// Ends a program and, where there are process groups, everything in its group.
#[cfg(unix)]
fn end(child: &mut Child) {
    // SAFETY: kill takes a process group id and a signal number and touches no memory.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn end(child: &mut Child) {
    let _ = child.kill();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Option<Duration>) -> Outcome {
        run(&Command {
            program: "sh".into(),
            args: vec!["-c".into(), script.into()],
            dir: ".".into(),
            timeout,
            ..Command::default()
        })
    }

    #[test]
    fn output_and_the_exit_status_are_kept() {
        let done = sh("echo out; echo err >&2; exit 3", None);
        assert_eq!(
            (done.stdout.as_str(), done.stderr.as_str(), done.code, done.timed_out),
            ("out\n", "err\n", Some(3), false)
        );
        assert!(run(&Command { program: "/no/such/program".into(), ..Command::default() }).error.is_some());
    }

    #[test]
    fn a_program_that_overruns_is_ended_with_what_it_started() {
        let started = Instant::now();
        let late = sh("sleep 30 & wait", Some(Duration::from_millis(200)));
        assert!(late.timed_out && late.code.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
