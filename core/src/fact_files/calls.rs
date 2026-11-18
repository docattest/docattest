//! The calls with which a fact expression reads a project file, `manifest("<path>")` and
//! `file("<path>")`, found in the expression as text: the one thing the core finds in an
//! expression, so that it can read the file before anything runs.

use std::sync::OnceLock;

/// The function a fact reads a project file with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// `manifest("<path>")`: the file's data, parsed by its format.
    Manifest,
    /// `file("<path>")`: the file's text.
    File,
}

impl Reader {
    /// The function's name, as a fact calls it and a message names what it read.
    pub fn name(self) -> &'static str {
        match self {
            Reader::Manifest => "manifest",
            Reader::File => "file",
        }
    }

    /// How a page reads `file` with this function: the call a fact makes.
    pub fn call(self, file: &str) -> String {
        format!("{}(\"{file}\")", self.name())
    }

    fn named(name: &str) -> Option<Reader> {
        [Reader::Manifest, Reader::File].into_iter().find(|reader| reader.name() == name)
    }
}

/// The paths an expression passes to `manifest` or `file` as a string literal, each with
/// the function, in any of the quotes the adapters' languages write one with. A call that
/// is a member of something else (`x.manifest("…")`) is not the one docattest gives; one
/// after a spread (`...`) or a range (`..`) is, since two dots are never member access. A
/// template that fills in a value (`` `${dir}/a.json` ``) is built at run time, so it names
/// no path here.
pub fn named_in(expression: &str) -> impl Iterator<Item = (Reader, &str)> {
    static CALL: OnceLock<fancy_regex::Regex> = OnceLock::new();
    let call = CALL.get_or_init(|| {
        fancy_regex::Regex::new(
            r#"(?:(?<=\.\.)|(?<![\w$.]))(manifest|file)\(\s*(["'`])([^"'`\\\r\n]*)\2\s*\)"#,
        )
        .expect("the pattern of a call that reads a file is valid")
    });
    call.captures_iter(expression).filter_map(|c| {
        let c = c.ok()?;
        let reader = Reader::named(c.get(1)?.as_str())?;
        let path = c.get(3)?.as_str();
        let filled_in = c.get(2)?.as_str() == "`" && path.contains("${");
        (!filled_in).then_some((reader, path))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(expression: &str) -> Vec<&str> {
        named_in(expression).map(|(_, path)| path).collect()
    }

    #[test]
    fn a_file_is_named_by_a_literal_path_in_any_quotes() {
        assert_eq!(named(r#"manifest("package.json").engines.node"#), ["package.json"]);
        assert_eq!(named("manifest('pyproject.toml')['project']"), ["pyproject.toml"]);
        assert_eq!(named("manifest( `go.mod` ).go + manifest(\"a/b.json\")"), ["go.mod", "a/b.json"]);
        assert_eq!(named("manifest(path)"), Vec::<&str>::new());
        assert_eq!(named(r#"loader.manifest("x.json")"#), Vec::<&str>::new());
        assert_eq!(named(r#"my_manifest("x.json")"#), Vec::<&str>::new());
        // A spread, or a range in Rust, is not member access.
        assert_eq!(named(r#"[...manifest("tsconfig.json").references].length"#), ["tsconfig.json"]);
        assert_eq!(named(r#"0..manifest("n.json")["n"]"#), ["n.json"]);
        // Quotes that do not match name nothing. The apostrophe is written \u{27}: a bare one
        // in a string reads as the start of a character to tools that measure this file.
        assert_eq!(named("manifest(\"x.json\u{27} )"), Vec::<&str>::new());
        // A template that fills in a value is built at run time; one that does not is a literal.
        assert_eq!(named("manifest(`${dir}/package.json`)"), Vec::<&str>::new());
        assert_eq!(named("manifest(`a/${'$'}b.json`)"), Vec::<&str>::new());
        assert_eq!(named("manifest(\"${dir}.json\")"), ["${dir}.json"]);
    }

    #[test]
    fn file_follows_the_same_rule_and_says_which_function_read_the_path() {
        let calls: Vec<(Reader, &str)> =
            named_in(r#"file("Dockerfile").includes("USER") && manifest('Dockerfile.json').user"#).collect();
        assert_eq!(calls, [(Reader::File, "Dockerfile"), (Reader::Manifest, "Dockerfile.json")]);
        assert_eq!(named(r#"os.file("a.txt") + profile("b.txt") + file(name)"#), Vec::<&str>::new());
        assert_eq!(named("file(`${dir}/Dockerfile`)"), Vec::<&str>::new());
        assert_eq!(Reader::File.call("Dockerfile"), r#"file("Dockerfile")"#);
        assert_eq!(Reader::Manifest.call("p.json"), r#"manifest("p.json")"#);
    }
}
