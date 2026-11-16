//! The one place the core reads git.
//!
//! Git runs as a child process from the project root, and only reads:
//! nothing is checked out, stored or written. [`base`] reads the project as committed at a
//! ref, for `check --base`; [`change`] reads the working tree's difference from a commit,
//! for `impact`. Both start git through [`run`], which starts it through
//! [`crate::process::spawn`] like every other child: under the spawn lock, so git may
//! run on any thread at any time, also while the adapter is being started.

pub mod base;
pub mod change;

use crate::files::text_file::decode;
use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::process::spawn;

/// Why a git command gave nothing.
pub(crate) enum Failure {
    /// Git could not be started because it (or the directory) does not exist.
    Missing,
    /// It ran and failed, or could not be run for another reason: what it wrote to stderr,
    /// or why.
    Failed(String),
}

/// Runs git with `args` in `root`, `input` on its stdin: what git wrote to stdout.
pub(crate) fn run(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, Failure> {
    let mut command = Command::new("git");
    command.args(args).current_dir(root).stdout(Stdio::piped()).stderr(Stdio::piped());
    command.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = spawn(&mut command).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Failure::Missing,
        _ => Failure::Failed(e.to_string()),
    })?;
    let stdin = child.stdin.take();
    // Git answers while it reads, so the input is written from another thread.
    let output = std::thread::scope(|scope| {
        if let (Some(mut stdin), Some(input)) = (stdin, input) {
            scope.spawn(move || {
                let _ = stdin.write_all(input);
            });
        }
        child.wait_with_output()
    });
    match output {
        Ok(output) if output.status.success() => Ok(output.stdout),
        Ok(output) => Err(Failure::Failed(decode(output.stderr))),
        Err(e) => Err(Failure::Failed(e.to_string())),
    }
}

/// The contents of several objects, read by one `git cat-file --batch`: one answer for each
/// of `objects` (a blob id, or `<commit>:<path>`; none may hold a newline), in order. `None`
/// where the object is missing or is not a blob. Shorter than `objects` when git stopped
/// early.
pub(crate) fn blobs(root: &Path, objects: &[String]) -> Result<Vec<Option<String>>, Failure> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let input = format!("{}\n", objects.join("\n"));
    let out = run(root, &["cat-file", "--batch"], Some(input.as_bytes()))?;
    Ok(answers(&out, objects.len()))
}

/// Up to `count` answers of `git cat-file --batch`, cut out of its output.
fn answers(out: &[u8], count: usize) -> Vec<Option<String>> {
    let mut reader = out;
    let mut found = Vec::with_capacity(count);
    while found.len() < count {
        match answer(&mut reader) {
            Some(blob) => found.push(blob),
            None => break,
        }
    }
    found
}

/// One answer of `git cat-file --batch`: `<oid> <type> <size>\n<contents>\n`, or a line
/// saying the object is missing. None at the end of the stream.
fn answer(reader: &mut impl BufRead) -> Option<Option<String>> {
    let mut header = Vec::new();
    if reader.read_until(b'\n', &mut header).ok()? == 0 {
        return None;
    }
    let header = String::from_utf8_lossy(&header);
    let mut parts = header.trim_end_matches('\n').split(' ');
    let (Some(oid), Some(kind), Some(size), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Some(None);
    };
    let (true, Ok(size)) = (oid.bytes().all(|b| b.is_ascii_hexdigit()), size.parse::<u64>()) else {
        return Some(None);
    };
    let mut contents = Vec::new();
    reader.by_ref().take(size).read_to_end(&mut contents).ok()?;
    let mut newline = [0u8; 1];
    reader.read_exact(&mut newline).ok()?;
    if contents.len() as u64 != size {
        return None;
    }
    Some((kind == "blob").then(|| decode(contents)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// A throwaway directory under the system's temporary directory, removed when dropped.
    pub(crate) struct Temp(pub PathBuf);

    impl Temp {
        pub(crate) fn new(label: &str) -> Temp {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("docattest-core-{label}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            // The system's temporary directory may be a symbolic link (or, on Windows, a short
            // name such as RUNNER~1); git prints real paths.
            Temp(real_path(&dir))
        }

        pub(crate) fn write(&self, file: &str, text: &str) {
            let path = self.0.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    /// The real path of `dir`, as a user or git would write it. On Windows, canonicalising
    /// answers in the verbatim form (`\\?\C:\…`), which git and `sh` cannot read and nobody
    /// types, so a drive path loses that prefix.
    fn real_path(dir: &Path) -> PathBuf {
        let real = fs::canonicalize(dir).unwrap();
        let plain = real.to_str().and_then(|text| text.strip_prefix("\\\\?\\"));
        match plain {
            Some(drive) if cfg!(windows) && !drive.starts_with("UNC\\") => PathBuf::from(drive),
            _ => real,
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Run a git command that writes, for a test's repository.
    pub(crate) fn git(root: &Path, args: &[&str]) {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let status = spawn(&mut command).and_then(|mut child| child.wait()).unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// A repository with `files` committed.
    pub(crate) fn repository(files: &[(&str, &str)]) -> Temp {
        let temp = Temp::new("git");
        for (file, text) in files {
            temp.write(file, text);
        }
        git(&temp.0, &["init", "-q", "-b", "main"]);
        git(&temp.0, &["add", "-A"]);
        git(&temp.0, &["commit", "-q", "-m", "base"]);
        temp
    }

    #[test]
    fn blobs_are_cut_out_of_the_batch_by_their_sizes() {
        let out = b"a blob 3\nx\ny\nb blob 0\n\nc blob 5\n\xff\xfe\n\nz\n";
        let text = |s: &str| Some(s.to_string());
        assert_eq!(answers(out, 3), vec![text("x\ny"), text(""), text("\u{FFFD}\u{FFFD}\n\nz")]);
        // A missing object, and one that is not a blob, are answered with nothing.
        assert_eq!(answers(b"a missing\nb tree 2\nxy\nc blob 1\nz\n", 3), vec![None, None, text("z")]);
        // Git stopped early: fewer answers than objects, and a cut one is not an answer.
        assert_eq!(answers(b"a blob 1\nx\n", 2), vec![text("x")]);
        assert_eq!(answers(b"a blob 9\nx\n", 1), Vec::new());
        assert_eq!(answers(b"", 1), Vec::new());
    }

    #[test]
    fn git_runs_from_many_threads_at_once_and_every_read_ends() {
        let repo = repository(&[("a.txt", "a"), ("b.txt", "b")]);
        let root = repo.0.as_path();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(move || {
                    for _ in 0..10 {
                        let objects = vec!["HEAD:a.txt".to_string(), "HEAD:b.txt".to_string()];
                        let read = blobs(root, &objects).unwrap_or_default();
                        assert_eq!(read, vec![Some("a".to_string()), Some("b".to_string())]);
                    }
                });
            }
        });
    }
}
