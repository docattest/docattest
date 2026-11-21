//! The command line: positionals anywhere; options from one table, as `--name value`,
//! `--name=value` or short groups (`-abc`); `--` ends the options. Strict: an unknown option,
//! a string option without a value and a boolean given one are usage errors. The rules and
//! the error messages are those of Node's `util.parseArgs` in strict mode, word for word,
//! because the recordings hold them.

use crate::api::errors::{Error, Result};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Type {
    String,
    Boolean,
}

/// One option: its long name, its type, its short name, and where its value goes.
struct Spec {
    name: &'static str,
    kind: Type,
    short: Option<char>,
    set: fn(&mut Options, Option<String>),
}

/// Options by long name.
const OPTIONS: [Spec; 17] = [
    Spec { name: "root", kind: Type::String, short: None, set: |o, v| o.root = v },
    Spec { name: "json", kind: Type::Boolean, short: None, set: |o, _| o.json = true },
    Spec { name: "quiet", kind: Type::Boolean, short: None, set: |o, _| o.quiet = true },
    Spec { name: "surface", kind: Type::String, short: None, set: |o, v| o.surface = v },
    Spec { name: "global", kind: Type::Boolean, short: None, set: |o, _| o.global = true },
    Spec { name: "local", kind: Type::Boolean, short: None, set: |o, _| o.local = true },
    Spec { name: "force", kind: Type::Boolean, short: None, set: |o, _| o.force = true },
    Spec { name: "base", kind: Type::String, short: None, set: |o, v| o.base = v },
    Spec { name: "report", kind: Type::String, short: None, set: |o, v| o.reports.extend(v) },
    Spec { name: "run-cited", kind: Type::Boolean, short: None, set: |o, _| o.run_cited = true },
    Spec { name: "hook", kind: Type::Boolean, short: None, set: |o, _| o.hook = true },
    Spec { name: "file", kind: Type::String, short: None, set: |o, v| o.file = v },
    Spec { name: "agent", kind: Type::String, short: None, set: |o, v| o.agent = v },
    Spec { name: "git", kind: Type::Boolean, short: None, set: |o, _| o.git = true },
    Spec {
        name: "fail-on-survivors",
        kind: Type::Boolean,
        short: None,
        set: |o, _| o.fail_on_survivors = true,
    },
    Spec { name: "help", kind: Type::Boolean, short: Some('h'), set: |o, _| o.help = true },
    Spec { name: "version", kind: Type::Boolean, short: None, set: |o, _| o.version = true },
];

/// Parsed options shared by every command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// As given; resolved against the working directory by the caller.
    pub root: Option<String>,
    pub json: bool,
    pub quiet: bool,
    pub surface: Option<String>,
    pub global: bool,
    /// `hooks install --local`: write to the agent's untracked local settings file.
    pub local: bool,
    pub force: bool,
    pub base: Option<String>,
    /// `check --report <format>=<path>`, once for each file, in the order given.
    pub reports: Vec<String>,
    /// `check --run-cited`: also run the tests that citations name.
    pub run_cited: bool,
    pub hook: bool,
    /// `impact --file`: the file that was edited.
    pub file: Option<String>,
    /// The coding agent: a shipped descriptor's name, or a descriptor file.
    pub agent: Option<String>,
    /// `hooks install --git`: the git pre-commit hook instead of an agent's hooks.
    pub git: bool,
    /// `mutate --fail-on-survivors`: exit 1 when a proof survived every mutant.
    pub fail_on_survivors: bool,
    pub help: bool,
    pub version: bool,
}

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    pub command: Option<String>,
    pub args: Vec<String>,
    pub options: Options,
}

struct Token {
    name: String,
    raw: String,
    value: Option<String>,
    inline: bool,
}

enum Item {
    Option(Token),
    Positional(String),
}

fn option(name: &str) -> Option<&'static Spec> {
    OPTIONS.iter().find(|spec| spec.name == name)
}

fn long_for_short(short: char) -> String {
    OPTIONS
        .iter()
        .find(|spec| spec.short == Some(short))
        .map_or_else(|| short.to_string(), |spec| spec.name.to_string())
}

fn is_string(name: &str) -> bool {
    option(name).is_some_and(|spec| spec.kind == Type::String)
}

/// Parse arguments; unknown options are a usage error.
pub fn parse_command_line(argv: &[String]) -> Result<CommandLine> {
    let items = tokens(&with_optional_surface(argv));
    let mut options = Options::default();
    let mut positionals = Vec::new();
    for item in items {
        match item {
            Item::Positional(p) => positionals.push(p),
            Item::Option(token) => {
                check_usage(&token)?;
                if let Some(spec) = option(&token.name) {
                    (spec.set)(&mut options, token.value.clone());
                }
            }
        }
    }
    let mut positionals = positionals.into_iter();
    let command = positionals.next();
    Ok(CommandLine { command, args: positionals.collect(), options })
}

// `uncovered --surface` with no name means the default surface, as the docs write it.
fn with_optional_surface(argv: &[String]) -> Vec<String> {
    argv.iter()
        .enumerate()
        .filter(|(k, arg)| *arg != "--surface" || argv.get(k + 1).is_some_and(|next| !next.starts_with('-')))
        .map(|(_, a)| a.clone())
        .collect()
}

fn tokens(argv: &[String]) -> Vec<Item> {
    let mut reader = ArgReader { remaining: argv.iter().cloned().collect(), out: Vec::new() };
    while let Some(arg) = reader.remaining.pop_front() {
        if arg == "--" {
            reader.out.extend(reader.remaining.drain(..).map(Item::Positional));
            break;
        }
        reader.item(arg);
    }
    reader.out
}

/// The tokenizer: the arguments still to read, and the items read.
struct ArgReader {
    remaining: std::collections::VecDeque<String>,
    out: Vec<Item>,
}

impl ArgReader {
    fn item(&mut self, arg: String) {
        let chars: Vec<char> = arg.chars().collect();
        let short = chars.len() > 1 && chars[0] == '-' && chars[1] != '-';
        if short && chars.len() == 2 {
            self.short_option(chars[1], arg);
        } else if short {
            self.short_group(&chars);
        } else if let Some(long) = arg.strip_prefix("--")
            && !long.is_empty()
        {
            self.long_option(long, &arg);
        } else {
            self.out.push(Item::Positional(arg));
        }
    }

    /// `-x`, and the value after it for a string option.
    fn short_option(&mut self, short: char, raw: String) {
        let name = long_for_short(short);
        let value = if is_string(&name) { self.remaining.pop_front() } else { None };
        self.out.push(Item::Option(Token { name, raw, value, inline: false }));
    }

    /// `-abc`: a group of short options, or a short string option with its value.
    fn short_group(&mut self, chars: &[char]) {
        let first = long_for_short(chars[1]);
        if !is_string(&first) {
            // A group of short options: `-abc` is `-a -b -c`.
            for e in expand_group(chars).into_iter().rev() {
                self.remaining.push_front(e);
            }
            return;
        }
        // `-fVALUE` for a string option with a short name; there is none.
        let name = first;
        let value: String = chars[2..].iter().collect();
        self.out.push(Item::Option(Token {
            name,
            raw: format!("-{}", chars[1]),
            value: Some(value),
            inline: true,
        }));
    }

    /// `--name`, with the value after it for a string option, or `--name=value`.
    fn long_option(&mut self, long: &str, arg: &str) {
        match long.find('=') {
            None => {
                let value = if is_string(long) { self.remaining.pop_front() } else { None };
                self.out.push(Item::Option(Token {
                    name: long.to_string(),
                    raw: arg.to_string(),
                    value,
                    inline: false,
                }));
            }
            Some(eq) => {
                let name = &long[..eq];
                self.out.push(Item::Option(Token {
                    name: name.to_string(),
                    raw: format!("--{name}"),
                    value: Some(long[eq + 1..].to_string()),
                    inline: true,
                }));
            }
        }
    }
}

/// `-abc` as `-a`, `-b`, `-c`; a string option takes the rest of the group as its value.
fn expand_group(chars: &[char]) -> Vec<String> {
    let mut expanded = Vec::new();
    for k in 1..chars.len() {
        let name = long_for_short(chars[k]);
        if !is_string(&name) || k == chars.len() - 1 {
            expanded.push(format!("-{}", chars[k]));
        } else {
            expanded.push(format!("-{}", chars[k..].iter().collect::<String>()));
            break;
        }
    }
    expanded
}

fn check_usage(token: &Token) -> Result<()> {
    let Some(&Spec { kind, short, .. }) = option(&token.name) else {
        let quoted = serde_json::to_string(&token.raw).unwrap_or_default();
        return Err(Error::user(format!(
            "Unknown option '{}'. To specify a positional argument starting with a '-', place it at the end of the command after '--', as in '-- {quoted}",
            token.raw
        )));
    };
    let both = match short {
        Some(s) => format!("-{s}, --{}", token.name),
        None => format!("--{}", token.name),
    };
    match (kind, &token.value) {
        (Type::String, None) => return Err(Error::user(format!("Option '{both} <value>' argument missing"))),
        (Type::Boolean, Some(_)) => {
            return Err(Error::user(format!("Option '{both}' does not take an argument")));
        }
        _ => {}
    }
    if let Some(value) = &token.value
        && !token.inline
        && value.chars().count() > 1
        && value.starts_with('-')
    {
        let raw = &token.raw;
        let example =
            if raw.starts_with("--") { format!("{raw}=-XYZ") } else { format!("--{} -XYZ", token.name) };
        return Err(Error::user(format!(
            "Option '{raw}' argument is ambiguous.\nDid you forget to specify the option argument for '{raw}'?\nTo specify an option argument starting with a dash use '{example}'."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(args: &[&str]) -> String {
        let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        match parse_command_line(&argv) {
            Err(Error::User(m)) => m,
            other => format!("{other:?}"),
        }
    }

    fn line(args: &[&str]) -> CommandLine {
        let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_command_line(&argv).unwrap()
    }

    #[test]
    fn messages_are_nodes() {
        assert_eq!(
            err(&["--foo"]),
            "docattest: Unknown option '--foo'. To specify a positional argument starting with a '-', place it at the end of the command after '--', as in '-- \"--foo\""
        );
        assert_eq!(err(&["--json=1"]), "docattest: Option '--json' does not take an argument");
        assert_eq!(err(&["--root"]), "docattest: Option '--root <value>' argument missing");
        assert_eq!(
            err(&["--root", "--json"]),
            "docattest: Option '--root' argument is ambiguous.\nDid you forget to specify the option argument for '--root'?\nTo specify an option argument starting with a dash use '--root=-XYZ'."
        );
        assert_eq!(
            err(&["-hx"]),
            "docattest: Unknown option '-x'. To specify a positional argument starting with a '-', place it at the end of the command after '--', as in '-- \"-x\""
        );
        assert_eq!(err(&["--help=x"]), "docattest: Option '-h, --help' does not take an argument");
        assert_eq!(err(&["--foo=bar"]).split('.').next(), Some("docattest: Unknown option '--foo'"));
    }

    #[test]
    fn positionals_and_values() {
        let l = line(&["check", "docs/a.md", "--json", "--root", "-"]);
        assert_eq!(l.command.as_deref(), Some("check"));
        assert_eq!(l.args, vec!["docs/a.md"]);
        assert_eq!(l.options.root.as_deref(), Some("-"));
        assert!(l.options.json);
        let l = line(&["uncovered", "--surface"]);
        assert_eq!(l.options.surface, None);
        let l = line(&["--", "-h"]);
        assert_eq!(l.command.as_deref(), Some("-h"));
        assert!(line(&["-hh"]).options.help);
        let l = line(&["check", "--report", "junit=a.xml", "--report=codequality=b.json"]);
        assert_eq!(l.options.reports, vec!["junit=a.xml", "codequality=b.json"]);
        assert!(line(&["check", "--run-cited"]).options.run_cited);
        assert!(!line(&["check"]).options.run_cited);
    }
}
