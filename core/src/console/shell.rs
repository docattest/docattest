//! Running one command of a console block in `sh`, with a time limit.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

/// What a command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ran {
    /// It ended by itself. `code` is None when a signal ended it.
    Exited { code: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8> },
    /// It was still running, or something it started still held its output, at the limit; it
    /// was stopped, with everything it started.
    TimedOut,
}

/// How a command is run: where, for how long, and with which variables added to docattest's
/// own environment.
pub struct Shell<'a> {
    pub cwd: &'a Path,
    pub timeout: Duration,
    pub env: &'a [(&'a str, String)],
}

/// A word of a POSIX shell command: as it is when nothing in it needs quoting.
pub fn shell_word(text: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c);
    if !text.is_empty() && text.chars().all(plain) {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// What each of the three things a run waits for sent when it was done.
enum Done {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Status(std::io::Result<ExitStatus>),
}

impl Shell<'_> {
    /// Run `script` with `sh -c`, stdin empty. Its output is read to the end, so a command
    /// that leaves something running in the background which holds it runs until the limit.
    pub fn run(&self, script: &str) -> std::io::Result<Ran> {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script).current_dir(self.cwd);
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // A trace is for the run that was asked for; one in a command's output would be compared.
        command.env_remove(crate::trace::VARIABLE).envs(self.env.iter().map(|(k, v)| (k, v)));
        in_a_group_of_its_own(&mut command);
        let child = crate::process::spawn(&mut command)?;
        let group = child.id();
        let (sender, receiver) = channel();
        watch(child, &sender);
        match collect(&receiver, Instant::now() + self.timeout)? {
            Some(ran) => Ok(ran),
            None => {
                stop_group(group)?;
                Ok(Ran::TimedOut)
            }
        }
    }
}

/// Read both outputs and wait for the exit, each on a thread of its own, so that neither
/// pipe fills while the other is read. A send fails only once the run has stopped
/// listening, at its limit.
fn watch(mut child: Child, sender: &Sender<Done>) {
    read_all(child.stdout.take(), Done::Stdout, sender.clone());
    read_all(child.stderr.take(), Done::Stderr, sender.clone());
    let sender = sender.clone();
    std::thread::spawn(move || {
        let _ = sender.send(Done::Status(child.wait()));
    });
}

fn read_all<R: Read + Send + 'static>(pipe: Option<R>, done: fn(Vec<u8>) -> Done, sender: Sender<Done>) {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            // A read that fails ends the output where it failed: what was read is what the
            // command printed, as far as anyone can tell.
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = sender.send(done(bytes));
    });
}

/// The run, once both outputs have ended and the command has exited; None at the deadline.
fn collect(receiver: &Receiver<Done>, deadline: Instant) -> std::io::Result<Option<Ran>> {
    let (mut stdout, mut stderr, mut status) = (None, None, None);
    while stdout.is_none() || stderr.is_none() || status.is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(left) {
            Ok(Done::Stdout(bytes)) => stdout = Some(bytes),
            Ok(Done::Stderr(bytes)) => stderr = Some(bytes),
            Ok(Done::Status(exit)) => status = Some(exit?),
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(std::io::Error::other("a command's watcher ended without a word"));
            }
        }
    }
    let code = status.and_then(|s| s.code());
    Ok(Some(Ran::Exited { code, stdout: stdout.unwrap_or_default(), stderr: stderr.unwrap_or_default() }))
}

/// The command leads a process group, so that stopping it stops everything it started.
#[cfg(unix)]
fn in_a_group_of_its_own(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn in_a_group_of_its_own(_: &mut Command) {}

/// Stop every process in the group `group` leads. `kill` does it, as the core has no other
/// way to signal a group without `unsafe`.
fn stop_group(group: u32) -> std::io::Result<()> {
    let mut kill = Command::new("kill");
    kill.args(["-KILL", "--", &format!("-{group}")]).stdout(Stdio::null()).stderr(Stdio::null());
    crate::process::spawn(&mut kill)?.wait()?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn shell(timeout_ms: u64) -> Shell<'static> {
        Shell { cwd: Path::new("/"), timeout: Duration::from_millis(timeout_ms), env: &[] }
    }

    #[test]
    fn a_command_gives_its_two_outputs_and_its_code() {
        let ran = shell(5000).run("echo out; echo err >&2; exit 3").unwrap();
        assert_eq!(ran, Ran::Exited { code: Some(3), stdout: b"out\n".to_vec(), stderr: b"err\n".to_vec() });
    }

    #[test]
    fn a_command_is_stopped_at_the_limit_with_what_it_started() {
        let started = Instant::now();
        assert_eq!(shell(200).run("sleep 30 & sleep 30").unwrap(), Ran::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn variables_are_added() {
        let env = [("DOCATTEST_GIVEN", "a b".to_string())];
        let shell = Shell { env: &env, ..shell(5000) };
        let Ran::Exited { stdout, .. } = shell.run("echo \"$DOCATTEST_GIVEN\"").unwrap() else { panic!() };
        assert_eq!(stdout, b"a b\n");
    }

    #[test]
    fn a_shell_word_is_quoted_only_when_it_must_be() {
        assert_eq!(shell_word("agents/my-agent.json"), "agents/my-agent.json");
        assert_eq!(shell_word("my agent.json"), "'my agent.json'");
        assert_eq!(shell_word("it's"), "'it'\\''s'");
        assert_eq!(shell_word(""), "''");
        assert_eq!(shell_word("$HOME"), "'$HOME'");
    }
}
