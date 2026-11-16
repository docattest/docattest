//! The source files of the adapter's language, as its `hello` states them (PROTOCOL.md §3).
//!
//! The core never knows a language's file names. The adapter gives two lists of globs; a
//! project file is a source file when its path matches one of `include` and none of
//! `exclude`. They are read exactly as the config's own globs are: against the whole
//! project-relative path, dotfiles matching, case as written.

use serde_json::Value;

use crate::files::glob::GlobMatcher;

/// Which project files are source files of a language. The default has none.
#[derive(Debug, Default)]
pub struct SourceFiles {
    include: Option<GlobMatcher>,
    exclude: Option<GlobMatcher>,
}

impl SourceFiles {
    /// From the globs themselves. `Err` says which glob cannot be read.
    pub fn new(include: &[String], exclude: &[String]) -> Result<SourceFiles, String> {
        let matcher = |key: &str, globs: &[String]| {
            GlobMatcher::new(globs)
                .map(Some)
                .map_err(|why| format!("\"sourceFiles.{key}\" has a glob that cannot be read: {why}"))
        };
        Ok(SourceFiles { include: matcher("include", include)?, exclude: matcher("exclude", exclude)? })
    }

    /// From the `sourceFiles` of a `hello` answer: `{ "include": string[], "exclude": string[] }`.
    pub fn from_hello(value: Option<&Value>) -> Result<SourceFiles, String> {
        let globs = |key: &str| -> Result<Vec<String>, String> {
            value
                .and_then(|v| v.get(key))
                .and_then(Value::as_array)
                .and_then(|list| list.iter().map(|g| g.as_str().map(str::to_string)).collect())
                .ok_or_else(|| format!("\"sourceFiles.{key}\" must be an array of globs"))
        };
        SourceFiles::new(&globs("include")?, &globs("exclude")?)
    }

    /// Whether the project-relative `path` (`/` separators, no `.` or `..` segments) is a
    /// source file of the language.
    pub fn is_match(&self, path: &str) -> bool {
        let matches = |globs: &Option<GlobMatcher>| globs.as_ref().is_some_and(|g| g.is_match(path));
        matches(&self.include) && !matches(&self.exclude)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// What the TypeScript adapter greets with; the core holds it nowhere but in tests.
    pub(crate) fn typescript() -> SourceFiles {
        let hello = json!({
            "include": ["**/*.{ts,tsx,mts,mtsx,cts,ctsx,js,jsx,mjs,mjsx,cjs,cjsx}"],
            "exclude": ["**/*.d.{ts,mts,cts}"],
        });
        SourceFiles::from_hello(Some(&hello)).unwrap()
    }

    #[test]
    fn a_source_file_matches_an_include_glob_and_no_exclude_glob() {
        let source = typescript();
        for file in
            ["a.ts", "src/a.tsx", "a.mjs", "a/b/c.cts", ".ts", "x.d.tsx", ".hidden/a.js", "a/.b/c.jsx"]
        {
            assert!(source.is_match(file), "{file}");
        }
        for file in ["a.d.ts", "src/a.d.mts", "a.d.cts", "a.md", "ts", "a.TS", "a.ts.map", "a.tsxx", ""] {
            assert!(!source.is_match(file), "{file}");
        }
    }

    #[test]
    fn globs_are_read_as_the_configs_are() {
        let globs = |list: &[&str]| list.iter().map(|g| g.to_string()).collect::<Vec<_>>();
        let source =
            SourceFiles::new(&globs(&["*.py", "./bin/**", "lib/**/*.PY"]), &globs(&["bin/**/*.txt"]))
                .unwrap();
        // The whole path, not the file name: a glob without a slash names top-level files.
        assert!(source.is_match("a.py") && !source.is_match("pkg/a.py"));
        // Dotfiles and hidden directories match; a leading "./" on a glob is dropped.
        assert!(source.is_match(".a.py") && source.is_match("bin/.x/run") && !source.is_match("bin/a.txt"));
        // Case is the path's own.
        assert!(source.is_match("lib/x/a.PY") && !source.is_match("lib/x/a.py") && !source.is_match("A.PY"));
    }

    #[test]
    fn no_globs_no_source_files_and_a_greeting_without_them_is_refused() {
        let none = SourceFiles::from_hello(Some(&json!({ "include": [], "exclude": [] }))).unwrap();
        assert!(!none.is_match("a.ts") && !SourceFiles::default().is_match("a.ts"));
        let refused = |value: Option<Value>| SourceFiles::from_hello(value.as_ref()).unwrap_err();
        assert_eq!(refused(None), "\"sourceFiles.include\" must be an array of globs");
        assert_eq!(
            refused(Some(json!({ "include": ["*.x"] }))),
            "\"sourceFiles.exclude\" must be an array of globs"
        );
        assert_eq!(
            refused(Some(json!({ "include": ["*.x", 1], "exclude": [] }))),
            "\"sourceFiles.include\" must be an array of globs"
        );
        assert!(
            refused(Some(json!({ "include": [""], "exclude": [] })))
                .starts_with("\"sourceFiles.include\" has a glob that cannot be read: ")
        );
    }
}
