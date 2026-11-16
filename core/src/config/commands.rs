//! `commands`: the programs the project's console blocks run, each with the files it is built
//! from, so `impact` can tell which blocks a change reaches. A command runs whatever it was
//! built into, which no language can trace from its source, so the config says it.

use super::Fields;

/// A program the console blocks run, by the name a command line calls it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// One word, as a command line writes it: `mytool`.
    pub name: String,
    /// Globs of the project files the program is built from.
    pub files: Vec<String>,
}

/// The commands the config lists, in its order; none when it lists none.
pub(super) fn read_commands(fields: &mut Fields) -> Vec<Command> {
    fields
        .nested("commands", |entries| {
            let names: Vec<String> = entries.object.keys().cloned().collect();
            names.into_iter().filter_map(|name| command(entries, name)).collect()
        })
        .unwrap_or_default()
}

fn command(entries: &mut Fields, name: String) -> Option<Command> {
    if name.is_empty() || name.contains(|c: char| c.is_whitespace() || c == '/') {
        let why = "a name is one word, as a command line calls the program";
        entries.problems.push(format!("\"commands\" has {name:?}, which is not a command's name: {why}"));
        return None;
    }
    let files = entries.globs(&name, entries.object.get(&name));
    Some(Command { name, files })
}

#[cfg(test)]
mod tests {
    use super::Command;
    use crate::config::parse_config;
    use serde_json::json;

    fn commands(value: serde_json::Value) -> Result<Vec<Command>, String> {
        let config =
            json!({ "pages": [], "surfaces": { "default": "x" }, "adapter": "x", "commands": value });
        parse_config(&config.to_string()).map(|c| c.commands).map_err(|e| e.to_string())
    }

    #[test]
    fn a_command_is_a_name_and_the_globs_of_its_files() {
        let read = commands(json!({ "mytool": ["src/**/*.rs"], "mytool-admin": ["admin/*.rs", "src/*.rs"] }));
        assert_eq!(
            read.unwrap(),
            vec![
                Command { name: "mytool".into(), files: vec!["src/**/*.rs".into()] },
                Command { name: "mytool-admin".into(), files: vec!["admin/*.rs".into(), "src/*.rs".into()] },
            ]
        );
        assert_eq!(commands(json!({})).unwrap(), vec![]);
    }

    #[test]
    fn a_name_that_is_not_one_word_or_files_that_are_not_globs_are_refused() {
        for (bad, said) in [
            (json!(["src/*.rs"]), "\"commands\" must be an object"),
            (
                json!({ "my tool": ["src/*.rs"] }),
                "\"commands\" has \"my tool\", which is not a command's name",
            ),
            (json!({ "bin/mytool": ["src/*.rs"] }), "which is not a command's name"),
            (json!({ "mytool": "src/*.rs" }), "\"commands.mytool\" must be an array of globs"),
            (json!({ "mytool": ["./"] }), "\"commands.mytool\" has a glob that cannot be read"),
        ] {
            let error = commands(bad.clone()).unwrap_err();
            assert!(error.contains(said), "{bad}: {error}");
        }
    }
}
