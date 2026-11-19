//! Test file aliases: `<!-- tests: <name> = <file> -->` names a test file once, so that the
//! citations on the page can write `<!-- test: <name> > "<title>" -->`. An alias holds on the
//! whole page, wherever it is written, and each name is declared once. A name is letters,
//! digits, `_` and `-`, so it never looks like a path.

use super::citation::cited_file;
use super::directives::Directive;
use crate::markdown::model::Keyword;
use crate::text::{is_js_space, trim};

/// A name a page gives a test file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub name: String,
    /// The test file, in canonical form, relative to the project root.
    pub file: String,
}

const FORM: &str = "expected <!-- tests: <name> = <file> -->";

/// Parse a comment body that starts with the word `tests`. None for an ordinary comment: one
/// with no colon after the word, or no `=` after the colon (`tests: none yet`).
pub fn parse_alias(text: &str, line: usize) -> Option<Directive> {
    let rest = text.strip_prefix("tests")?.trim_start_matches(is_js_space).strip_prefix(':')?;
    let (name, file) = rest.split_once('=')?;
    let invalid = |why: String| Directive::Invalid {
        as_kind: Keyword::Tests,
        message: format!("tests at L{line} {why}; {FORM}"),
    };
    let name = trim(name);
    if !is_alias_name(name) {
        return Some(invalid(format!(
            "names the alias {name:?}, which is not a name of letters, digits, _ and -"
        )));
    }
    let file = trim(file);
    // A citation writes its file bare too; refused here, a quote would be read as part of it.
    if file.contains(['"', '\'']) {
        return Some(invalid(format!("names its file in quotes, {file}: write the file unquoted")));
    }
    Some(match cited_file(file) {
        Ok(file) => Directive::Alias(Alias { name: name.to_string(), file }),
        Err(why) => invalid(why),
    })
}

/// A letter or `_` first, then letters, digits, `_` and `-`: never a path, which has a `/` or
/// a `.` in it wherever a test file is.
fn is_alias_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The aliases a page declares, each with the line it is declared on.
#[derive(Debug, Clone, Default)]
pub struct Aliases {
    declared: Vec<(Alias, usize)>,
}

impl Aliases {
    /// Declare an alias written on `line`; Err when the page already declares its name.
    pub fn declare(&mut self, alias: &Alias, line: usize) -> Result<(), String> {
        if let Some((_, first)) = self.declared.iter().find(|(known, _)| known.name == alias.name) {
            let name = &alias.name;
            return Err(format!("tests at L{line} declares {name} again: it is declared at L{first}"));
        }
        self.declared.push((alias.clone(), line));
        Ok(())
    }

    /// Each alias, with the line it is declared on, in the order declared.
    pub fn declared(&self) -> &[(Alias, usize)] {
        &self.declared
    }

    /// The alias a citation's file names, when it names one.
    pub fn named(&self, file: &str) -> Option<&Alias> {
        self.declared.iter().map(|(alias, _)| alias).find(|alias| alias.name == file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestations::directives::parse_directive;

    fn alias(name: &str, file: &str) -> Option<Directive> {
        Some(Directive::Alias(Alias { name: name.into(), file: file.into() }))
    }

    fn message(body: &str) -> String {
        match parse_directive(body, 2) {
            Some(Directive::Invalid { message, .. }) => message,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn an_alias_names_a_test_file() {
        assert_eq!(
            parse_directive(" tests: launch = ./agent/launch_test.go ", 1),
            alias("launch", "agent/launch_test.go")
        );
        assert_eq!(parse_directive("tests:run-2=t/run.test.ts", 1), alias("run-2", "t/run.test.ts"));
        assert_eq!(parse_directive(" tests: none yet ", 1), None);
        assert_eq!(parse_directive(" tests = a ", 1), None);
    }

    #[test]
    fn an_alias_that_looks_like_a_path_or_names_no_file_is_an_error() {
        let path = message("tests: agent/launch = a_test.go");
        assert_eq!(
            path,
            "tests at L2 names the alias \"agent/launch\", which is not a name of letters, digits, _ and -; \
             expected <!-- tests: <name> = <file> -->"
        );
        assert!(message("tests: launch.go = a_test.go").contains("names the alias \"launch.go\""));
        assert!(message("tests: = a_test.go").contains("names the alias \"\""));
        assert!(message("tests: launch = ").contains("tests at L2 names no file"));
        assert!(message("tests: launch = ../a_test.go").contains("not a path relative to the project root"));
        assert_eq!(
            message("tests: launch = \"a_test.go\""),
            "tests at L2 names its file in quotes, \"a_test.go\": write the file unquoted; \
             expected <!-- tests: <name> = <file> -->"
        );
        assert!(message("tests: launch = 'a_test.go'").contains("write the file unquoted"));
    }

    #[test]
    fn a_name_is_declared_once_on_a_page() {
        let mut aliases = Aliases::default();
        let launch = Alias { name: "launch".into(), file: "a_test.go".into() };
        assert_eq!(aliases.declare(&launch, 1), Ok(()));
        let again = Alias { name: "launch".into(), file: "b_test.go".into() };
        assert_eq!(
            aliases.declare(&again, 7),
            Err("tests at L7 declares launch again: it is declared at L1".into())
        );
        assert_eq!(aliases.named("launch"), Some(&launch));
        assert_eq!(aliases.named("a_test.go"), None);
    }
}
