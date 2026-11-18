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
    /// Ends the program, and everything it started, when it is given. It counts from the
    /// launch, or, for a program that says when its own code starts, from then.
    pub timeout: Option<Duration>,
    /// How the program says that its own code has started, when it does.
    pub start: Option<Start>,
    /// Variables set for the program, over the adapter's own environment.
    pub env: Vec<(String, String)>,
}

/// How a program says that its own code has started: it creates the file `signal`, which
/// the variable `variable` names. What comes before (the system loading and checking a
/// freshly linked program, which on a loaded machine can take seconds) is then not
/// counted in its time limit. A program that has not started this long (`within`) after
/// its launch is ended.
pub struct Start {
    pub variable: &'static str,
    /// A file that does not exist yet.
    pub signal: PathBuf,
    pub within: Duration,
}

/// Why a program was ended before it exited by itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stopped {
    /// It ran past its time limit.
    TimedOut,
    /// It did not say that its code had started within the time it was given to.
    NotStarted,
}

/// How a program ended.
#[derive(Debug, Default)]
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    /// The exit status; `None` when the program did not exit by itself.
    pub code: Option<i32>,
    pub stopped: Option<Stopped>,
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
    if let Some(start) = &command.start {
        process.env(start.variable, &start.signal);
    }
    process.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    own_group(&mut process);
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(err) => return Outcome { error: Some(err.to_string()), ..Outcome::default() },
    };
    let streams = (reader(child.stdout.take()), reader(child.stderr.take()));
    let (code, stopped) = wait(&mut child, command);
    let read = |stream: std::thread::JoinHandle<String>| stream.join().unwrap_or_default();
    Outcome { stdout: read(streams.0), stderr: read(streams.1), code, stopped, error: None }
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
fn wait(child: &mut Child, command: &Command) -> (Option<i32>, Option<Stopped>) {
    let mut clock = Clock::new(command);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (status.code(), None),
            Ok(None) => match clock.overdue() {
                Some(stopped) => {
                    end(child);
                    let _ = child.wait();
                    return (None, Some(stopped));
                }
                None => std::thread::sleep(Duration::from_millis(2)),
            },
            Err(_) => return (None, None),
        }
    }
}

/// When a running program's time is up.
struct Clock<'a> {
    limit: Option<Duration>,
    counted: Counted<'a>,
}

/// Where a running program's time limit counts from.
#[derive(Clone, Copy)]
enum Counted<'a> {
    /// From this moment: its launch, or when its code said it had started.
    From(Instant),
    /// Not yet: the program, launched then, has not said that its code has started.
    NotYet { launched: Instant, start: &'a Start },
}

impl<'a> Clock<'a> {
    fn new(command: &'a Command) -> Self {
        let launched = Instant::now();
        let counted = match &command.start {
            Some(start) => Counted::NotYet { launched, start },
            None => Counted::From(launched),
        };
        Clock { limit: command.timeout, counted }
    }

    /// Why the program is to be ended now, if it is.
    fn overdue(&mut self) -> Option<Stopped> {
        let now = Instant::now();
        let since = match self.counted {
            Counted::From(since) => since,
            Counted::NotYet { launched, start } if !start.signal.exists() => {
                return (now >= launched + start.within).then_some(Stopped::NotStarted);
            }
            Counted::NotYet { .. } => {
                self.counted = Counted::From(now);
                now
            }
        };
        self.limit.filter(|limit| now >= since + *limit).map(|_| Stopped::TimedOut)
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

    /// Runs a script that says when its own code starts by touching `$STARTED`.
    fn sh_starting(test: &str, script: &str, timeout: Duration, within: Duration) -> Outcome {
        let signal = std::env::temp_dir().join(format!("docattest-start-{}-{test}", std::process::id()));
        let _ = std::fs::remove_file(&signal);
        let ran = run(&Command {
            program: "sh".into(),
            args: vec!["-c".into(), script.into()],
            dir: ".".into(),
            timeout: Some(timeout),
            start: Some(Start { variable: "STARTED", signal: signal.clone(), within }),
            ..Command::default()
        });
        let _ = std::fs::remove_file(&signal);
        ran
    }

    #[test]
    fn output_and_the_exit_status_are_kept() {
        let done = sh("echo out; echo err >&2; exit 3", None);
        assert_eq!(
            (done.stdout.as_str(), done.stderr.as_str(), done.code, done.stopped),
            ("out\n", "err\n", Some(3), None)
        );
        assert!(run(&Command { program: "/no/such/program".into(), ..Command::default() }).error.is_some());
    }

    #[test]
    fn a_program_that_overruns_is_ended_with_what_it_started() {
        let started = Instant::now();
        let late = sh("sleep 30 & wait", Some(Duration::from_millis(200)));
        assert_eq!((late.stopped, late.code), (Some(Stopped::TimedOut), None));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_time_limit_counts_from_when_the_programs_code_starts() {
        let (limit, within) = (Duration::from_millis(300), Duration::from_secs(10));
        let slow_start = sh_starting("slow", "sleep 1; touch \"$STARTED\"; sleep 0.1", limit, within);
        assert_eq!((slow_start.stopped, slow_start.code), (None, Some(0)));
        let started = Instant::now();
        let late = sh_starting("late", "touch \"$STARTED\"; sleep 30 & wait", limit, within);
        assert_eq!((late.stopped, late.code), (Some(Stopped::TimedOut), None));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_program_that_never_starts_is_ended() {
        let started = Instant::now();
        let never = sh_starting("never", "sleep 30", Duration::from_secs(30), Duration::from_millis(200));
        assert_eq!((never.stopped, never.code), (Some(Stopped::NotStarted), None));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
