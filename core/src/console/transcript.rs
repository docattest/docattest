//! Reading a console block: its commands, and what the block says each one prints.

use crate::text::{is_js_space, lines};

/// What starts a command line.
pub const PROMPT: &str = "$ ";

/// One command of a console block, and what the block says it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The command's line in the block, from 1.
    pub line: usize,
    pub command: String,
    /// The lines it prints, as the block shows them (stdout's, then stderr's), without
    /// trailing whitespace or trailing blank lines.
    pub output: Vec<String>,
    /// The code it exits with: 0, unless the block's last line for it is `[exit N]`.
    pub exit: i32,
}

impl Step {
    /// Whether the command line calls the program `name`: one of its words is the name, or a
    /// path ending in it (`./bin/name`). Words are split where a shell splits words and
    /// commands; quotes are not read, so a name the line only passes on (`echo name`) counts
    /// too, which can only make `impact` list a block it did not need to.
    pub fn runs(&self, name: &str) -> bool {
        let words = self.command.split(|c: char| c.is_whitespace() || ";|&()`".contains(c));
        words
            .into_iter()
            .any(|word| word == name || word.strip_suffix(name).is_some_and(|at| at.ends_with('/')))
    }
}

/// Why a console block cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptError {
    /// The block holds no command at all.
    NoCommand,
    /// Output comes before the first command, at this line of the block.
    NotACommand(usize),
}

/// The commands of a console block, in order. Every line after a command, up to the next
/// one, is what it prints.
pub fn parse_transcript(code: &str) -> Result<Vec<Step>, TranscriptError> {
    let mut steps: Vec<Step> = Vec::new();
    for (index, line) in lines(code).into_iter().enumerate() {
        let line = line.trim_end_matches(is_js_space);
        if let Some(command) = line.strip_prefix(PROMPT) {
            steps.push(Step { line: index + 1, command: command.to_string(), output: Vec::new(), exit: 0 });
            continue;
        }
        match steps.last_mut() {
            Some(step) => step.output.push(line.to_string()),
            None if line.is_empty() => {}
            None => return Err(TranscriptError::NotACommand(index + 1)),
        }
    }
    if steps.is_empty() {
        return Err(TranscriptError::NoCommand);
    }
    for step in &mut steps {
        step.exit = take_exit(&mut step.output);
    }
    Ok(steps)
}

/// The exit code an output says its command ends with, removing the `[exit N]` line that
/// says so, and the blank lines that end the output.
fn take_exit(output: &mut Vec<String>) -> i32 {
    drop_trailing_blanks(output);
    let said = output.last().and_then(|last| last.strip_prefix("[exit ")?.strip_suffix(']')?.parse().ok());
    if let Some(code) = said {
        output.pop();
        drop_trailing_blanks(output);
        return code;
    }
    0
}

/// Blank lines at the end of an output are not part of it: a block may leave one between
/// two commands, and a command may end with one.
pub fn drop_trailing_blanks(output: &mut Vec<String>) {
    while output.last().is_some_and(String::is_empty) {
        output.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(line: usize, command: &str, output: &[&str], exit: i32) -> Step {
        let output = output.iter().map(|l| l.to_string()).collect();
        Step { line, command: command.to_string(), output, exit }
    }

    #[test]
    fn each_command_owns_the_lines_up_to_the_next() {
        let block = "$ echo a  \na \n\n$ false\n\n[exit 1]\n\n$ true\n$x\n[exit one]";
        assert_eq!(
            parse_transcript(block),
            Ok(vec![
                step(1, "echo a", &["a"], 0),
                step(4, "false", &[], 1),
                step(8, "true", &["$x", "[exit one]"], 0)
            ])
        );
    }

    #[test]
    fn an_exit_line_counts_only_last() {
        assert_eq!(parse_transcript("$ x\n[exit 2]\ny"), Ok(vec![step(1, "x", &["[exit 2]", "y"], 0)]));
        assert_eq!(parse_transcript("$ x\n[exit -1]"), Ok(vec![step(1, "x", &[], -1)]));
    }

    #[test]
    fn a_command_runs_each_program_it_names_as_a_word_or_a_path() {
        let runs = |command: &str| step(1, command, &[], 0).runs("mytool");
        for yes in ["mytool --help", "cd x && ./bin/mytool run", "A=1 npx mytool", "(mytool)", "x|mytool"] {
            assert!(runs(yes), "{yes}");
        }
        for no in ["mytools", "my-mytool", "cat mytool.txt", "bin/mytool2"] {
            assert!(!runs(no), "{no}");
        }
    }

    #[test]
    fn a_block_starts_with_a_command() {
        assert_eq!(parse_transcript("\n$ x"), Ok(vec![step(2, "x", &[], 0)]));
        assert_eq!(parse_transcript("\nout\n$ x"), Err(TranscriptError::NotACommand(2)));
        assert_eq!(parse_transcript("$"), Err(TranscriptError::NotACommand(1)));
        assert_eq!(parse_transcript(""), Err(TranscriptError::NoCommand));
        assert_eq!(parse_transcript("\n \n"), Err(TranscriptError::NoCommand));
    }
}
