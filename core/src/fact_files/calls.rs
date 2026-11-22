//! The calls with which a fact expression reads a project file, `manifest("<path>")`,
//! `file("<path>")`, `matched("<path>", "<regex>")` and `manifestField("<path>", "<keys>")`,
//! found in the expression as text: the one thing the core finds in an expression, so that
//! it can read the file before anything runs.

use std::sync::OnceLock;

/// How a fact reads a project file: as data or as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// The file's data, parsed by its format: `manifest` and `manifestField`.
    Manifest,
    /// The file's text: `file` and `matched`.
    File,
}

impl Reader {
    /// The function that gives what this reader reads, as a fact calls it.
    pub fn name(self) -> &'static str {
        match self {
            Reader::Manifest => "manifest",
            Reader::File => "file",
        }
    }

    /// How a page reads `file` with this reader: the call a fact makes.
    pub fn call(self, file: &str) -> String {
        format!("{}(\"{file}\")", self.name())
    }
}

/// One call of a fact that reads a project file, with its arguments as the language's
/// string literals give them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// `manifest("<path>")`: the file's data.
    Manifest { path: String },
    /// `file("<path>")`: the file's text.
    File { path: String },
    /// `matched("<path>", "<regex>")`: what the regex matches in the file's text.
    Matched { path: String, pattern: String },
    /// `manifestField("<path>", "<keys>")`: the value at a path of keys in the file's data.
    ManifestField { path: String, keys: String },
}

impl Call {
    /// The function's name, as a fact calls it and a message names what it read.
    pub fn function(&self) -> &'static str {
        match self {
            Call::Manifest { .. } => "manifest",
            Call::File { .. } => "file",
            Call::Matched { .. } => "matched",
            Call::ManifestField { .. } => "manifestField",
        }
    }

    /// The file it reads, as the fact writes it.
    pub fn path(&self) -> &str {
        match self {
            Call::Manifest { path } | Call::File { path } => path,
            Call::Matched { path, .. } | Call::ManifestField { path, .. } => path,
        }
    }

    /// Whether it reads the file as data or as text.
    pub fn reader(&self) -> Reader {
        match self {
            Call::Manifest { .. } | Call::ManifestField { .. } => Reader::Manifest,
            Call::File { .. } | Call::Matched { .. } => Reader::File,
        }
    }

    /// The call `function` makes with these arguments; None for a call of the wrong arity.
    fn of(function: &str, path: String, second: Option<String>) -> Option<Call> {
        match (function, second) {
            ("manifest", None) => Some(Call::Manifest { path }),
            ("file", None) => Some(Call::File { path }),
            ("matched", Some(pattern)) => Some(Call::Matched { path, pattern }),
            ("manifestField", Some(keys)) => Some(Call::ManifestField { path, keys }),
            _ => None,
        }
    }
}

/// A string literal, in any of the forms the adapters' languages write one with.
const LITERAL: &str = r##"r#"(?:[^"\r\n]|"(?!#))*"#|r"[^"\r\n]*"|r'[^'\r\n]*'|"(?:[^"\\\r\n]|\\.)*"|'(?:[^'\\\r\n]|\\.)*'|`[^`\r\n]*`"##;

/// The calls an expression makes to `manifest`, `file`, `matched` or `manifestField` with
/// every argument a string literal whose value the core can know ([`literal_value`]). A call
/// that is a member of something else (`x.manifest("…")`) is not the one docattest gives; one
/// after a spread (`...`) or a range (`..`) is, since two dots are never member access.
pub fn named_in(expression: &str) -> impl Iterator<Item = Call> + '_ {
    calls_in(expression)
}

/// Each call `expression` makes.
fn calls_in(expression: &str) -> impl Iterator<Item = Call> + '_ {
    static CALL: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let call = CALL.get_or_init(|| {
        let pattern = format!(
            r"(?:(?<=\.\.)|(?<![\w$.]))(manifest|file|matched|manifestField)\(\s*({LITERAL})\s*(?:,\s*({LITERAL})\s*)?\)"
        );
        fancy_regex::Regex::new(&pattern).expect("the pattern of a call that reads a file is valid")
    });
    call.captures_iter(expression).filter_map(|c| {
        let c = c.ok()?;
        let written = c.get(2)?;
        let path = literal_value(written.as_str())?;
        let second = match c.get(3) {
            Some(written) => Some(literal_value(written.as_str())?),
            None => None,
        };
        Call::of(c.get(1)?.as_str(), path, second)
    })
}

/// The value of a string literal, read by the rules on which every language docattest
/// supports agrees, so that the core reads what the adapter's language will: a raw literal
/// (`r"…"`, `r'…'`, `r#"…"#`, `` `…` ``) is its text; in a quoted one, `\\` is a backslash
/// and a backslash before the quote is the quote. Any other escape means something different
/// in each language, or nothing, and a template that fills in a value (`` `${dir}` ``) is
/// built at run time: such a literal has no value here, and the call names no file.
fn literal_value(written: &str) -> Option<String> {
    if let Some(raw) = written.strip_prefix("r#\"").and_then(|rest| rest.strip_suffix("\"#")) {
        return Some(raw.to_string());
    }
    if let Some(raw) = written.strip_prefix('r') {
        return Some(raw[1..raw.len() - 1].to_string());
    }
    let quote = written.chars().next()?;
    let inner = &written[1..written.len() - 1];
    if quote == '`' {
        return (!inner.contains("${")).then(|| inner.to_string());
    }
    unescaped(inner, quote)
}

/// `inner`, the text of a literal in `quote`s, with `\\` and `\<quote>` read; None when it
/// holds any other escape.
fn unescaped(inner: &str, quote: char) -> Option<String> {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(escaped) if escaped == '\\' || escaped == quote => out.push(escaped),
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(expression: &str) -> Vec<String> {
        named_in(expression).map(|call| call.path().to_string()).collect()
    }

    fn file(path: &str) -> Call {
        Call::File { path: path.into() }
    }

    fn manifest(path: &str) -> Call {
        Call::Manifest { path: path.into() }
    }

    #[test]
    fn a_file_is_named_by_a_literal_path_in_any_quotes() {
        assert_eq!(named(r#"manifest("package.json").engines.node"#), ["package.json"]);
        assert_eq!(named("manifest('pyproject.toml')['project']"), ["pyproject.toml"]);
        assert_eq!(named("manifest( `go.mod` ).go + manifest(\"a/b.json\")"), ["go.mod", "a/b.json"]);
        assert_eq!(named("manifest(path)"), Vec::<String>::new());
        assert_eq!(named(r#"loader.manifest("x.json")"#), Vec::<String>::new());
        assert_eq!(named(r#"my_manifest("x.json")"#), Vec::<String>::new());
        // A spread, or a range in Rust, is not member access.
        assert_eq!(named(r#"[...manifest("tsconfig.json").references].length"#), ["tsconfig.json"]);
        assert_eq!(named(r#"0..manifest("n.json")["n"]"#), ["n.json"]);
        // Quotes that do not match name nothing. The apostrophe is written \u{27}: a bare one
        // in a string reads as the start of a character to tools that measure this file.
        assert_eq!(named("manifest(\"x.json\u{27} )"), Vec::<String>::new());
        // A template that fills in a value is built at run time; one that does not is a literal.
        assert_eq!(named("manifest(`${dir}/package.json`)"), Vec::<String>::new());
        assert_eq!(named("manifest(`a/${'$'}b.json`)"), Vec::<String>::new());
        assert_eq!(named("manifest(\"${dir}.json\")"), ["${dir}.json"]);
    }

    #[test]
    fn file_follows_the_same_rule_and_says_which_function_read_the_path() {
        let calls: Vec<Call> =
            named_in(r#"file("Dockerfile").includes("USER") && manifest('Dockerfile.json').user"#).collect();
        assert_eq!(calls, [file("Dockerfile"), manifest("Dockerfile.json")]);
        assert_eq!(named(r#"os.file("a.txt") + profile("b.txt") + file(name)"#), Vec::<String>::new());
        assert_eq!(named("file(`${dir}/Dockerfile`)"), Vec::<String>::new());
        assert_eq!(Reader::File.call("Dockerfile"), r#"file("Dockerfile")"#);
        assert_eq!(Reader::Manifest.call("p.json"), r#"manifest("p.json")"#);
    }

    #[test]
    fn matched_and_field_take_a_second_literal_and_read_as_file_and_manifest_do() {
        let calls: Vec<Call> = named_in(
            r#"matched("Dockerfile", "USER (\\d+)") + manifestField(".gitlab-ci.yml", 'a."b.c".d')"#,
        )
        .collect();
        let matched = Call::Matched { path: "Dockerfile".into(), pattern: r"USER (\d+)".into() };
        let field = Call::ManifestField { path: ".gitlab-ci.yml".into(), keys: r#"a."b.c".d"#.into() };
        assert_eq!(calls, [matched.clone(), field.clone()]);
        assert_eq!([matched.reader(), field.reader()], [Reader::File, Reader::Manifest]);
        assert_eq!([matched.function(), field.function()], ["matched", "manifestField"]);
        // Each function takes its own number of arguments, every one a literal.
        assert_eq!(named(r#"file("a", "b") + manifest("c", "d") + manifestField("e", "f")"#), ["e"]);
        assert_eq!(
            named(r#"matched("a") + manifestField("b") + matched("c", pattern)"#),
            Vec::<String>::new()
        );
        assert_eq!(
            named(r#"x.matched("a", "b") + my_manifestField("c", "d") + field("e", "f")"#),
            Vec::<String>::new()
        );
    }

    fn second(expression: &str) -> Vec<String> {
        let pattern = |call: Call| match call {
            Call::Matched { pattern, .. } => Some(pattern),
            _ => None,
        };
        named_in(expression).filter_map(pattern).collect()
    }

    #[test]
    fn a_literal_is_read_by_the_rules_every_language_shares() {
        // Raw literals: Rust and Python's r"…", Rust's r#"…"#, Go's `…`.
        assert_eq!(second(r#"matched("D", r"USER (\d+)")"#), [r"USER (\d+)"]);
        assert_eq!(second(r#"matched("D", r'USER (\d+)')"#), [r"USER (\d+)"]);
        assert_eq!(second(r##"matched("D", r#"say "(\w+)""#)"##), [r#"say "(\w+)""#]);
        assert_eq!(second(r"matched(`D`, `USER (\d+)`)"), [r"USER (\d+)"]);
        // Quoted: a doubled backslash is one, and an escaped quote is the quote.
        assert_eq!(second(r#"matched("D", "USER (\\d+) \"x\"")"#), [r#"USER (\d+) "x""#]);
        assert_eq!(second(r"matched('D', 'it\'s (\\w)')"), [r"it's (\w)"]);
        // Any other escape differs between languages, so the call names nothing.
        assert_eq!(second(r#"matched("D", "USER (\d+)")"#), Vec::<String>::new());
        assert_eq!(second(r#"matched("D", "a\nb")"#), Vec::<String>::new());
        assert_eq!(named(r#"file("a\\b")"#), [r"a\b"]);
    }
}
