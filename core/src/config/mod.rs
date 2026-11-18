//! Reading and validating `docattest.config.json` (`src/config/`).
//!
//! The config is validated in one pass: every problem is reported at once, by key, in the
//! order the docs list the keys, and unknown keys are errors.

use crate::api::errors::{Error, Result};
use crate::files::glob::{GlobMatcher, is_negation};
use crate::files::project_files::ProjectFiles;
use crate::files::source::DiskFiles;
use crate::json::parse_json;
use crate::text::trim;
use serde_json::{Map, Value};

mod commands;
mod members;
mod undocumented;
pub use commands::Command;
pub use members::Member;

/// The config file, at the project root.
pub const CONFIG_FILE: &str = "docattest.config.json";
/// The surface every config must name, and the one a fact or an import uses unless it names another.
pub const DEFAULT_SURFACE: &str = "default";

const KEYS: [&str; 13] = [
    "$schema",
    "pages",
    "frozen",
    "sources",
    "surfaces",
    "setup",
    "snapshots",
    "adapter",
    "runtime",
    "ignoreTags",
    "members",
    "commands",
    "undocumented",
];
const RESERVED_TAGS: [&str; 3] = ["check", "compile", "network"];
/// What an entry may be. What a string names, or what is imported from which file, is the
/// adapter's to say, and it says so when it opens the project.
const SHAPE: &str = "a string, or { \"import\": \"<name>\", \"path\": \"<file>\" }";
/// Why a glob of files cannot start with `!`.
const FILE_NEGATION: &str = "a glob starting with \"!\" matches every file but the ones it names, so it cannot leave files out; \"frozen\" leaves pages out of the check";

/// Time budget for one block, one fact or one table row, in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;
/// Age after which a snapshot is reported as stale, in days.
pub const DEFAULT_SNAPSHOT_MAX_AGE_DAYS: u64 = 90;

/// A surface as written in the config: a string, or an object naming an import and a path.
/// What either means is the adapter's to say; the core passes it on as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceEntry {
    Specifier(String),
    Both { import: String, path: String },
}

impl SurfaceEntry {
    /// The entry as the config wrote it.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            SurfaceEntry::Specifier(written) => serde_json::json!(written),
            SurfaceEntry::Both { import, path } => serde_json::json!({ "import": import, "path": path }),
        }
    }
}

/// A validated config with every default filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectConfig {
    pub pages: Vec<String>,
    pub frozen: Vec<String>,
    pub sources: Vec<String>,
    /// In the order the config writes them.
    pub surfaces: Vec<(String, SurfaceEntry)>,
    pub setup: Option<String>,
    pub snapshots_dir: Option<String>,
    pub max_age_days: u64,
    pub adapter: String,
    pub timeout_ms: u64,
    pub workers: u64,
    pub ignore_tags: Vec<String>,
    /// The projects below this one that `check` checks with it.
    pub members: Vec<Member>,
    /// The programs the console blocks run, with the files each is built from.
    pub commands: Vec<Command>,
    /// By surface, in the config's order: globs of the symbols not meant to be documented.
    pub undocumented: Vec<(String, Vec<String>)>,
}

/// Read and validate the project's config.
pub(crate) fn load_config(files: &DiskFiles) -> Result<ProjectConfig> {
    if !files.exists(CONFIG_FILE) {
        return Err(Error::user(format!("no {CONFIG_FILE} in {}", files.root().display())));
    }
    parse_config(&files.read(CONFIG_FILE).map_err(Error::user)?)
}

/// A matcher of globs from a validated config, which has checked that each one compiles.
pub(crate) fn matcher(globs: &[String]) -> Result<GlobMatcher> {
    GlobMatcher::new(globs)
        .map_err(|why| Error::internal(format!("a glob the config accepted cannot be read: {why}")))
}

/// Validate the text of a config file.
pub fn parse_config(text: &str) -> Result<ProjectConfig> {
    let raw = parse_json(text).map_err(|m| Error::user(format!("{CONFIG_FILE} is not valid JSON: {m}")))?;
    let Value::Object(object) = &raw else {
        return Err(Error::user(format!("{CONFIG_FILE} must contain a JSON object")));
    };
    let mut fields = Fields { object, prefix: String::new(), problems: Vec::new() };
    fields.reject_unknown(&KEYS);
    let config = read_config(&mut fields);
    if !fields.problems.is_empty() {
        return Err(Error::user(format!("invalid {CONFIG_FILE}: {}", fields.problems.join("; "))));
    }
    Ok(config)
}

/// Collects problems while reading fields of one JSON object.
struct Fields<'a> {
    object: &'a Map<String, Value>,
    prefix: String,
    problems: Vec<String>,
}

impl Fields<'_> {
    fn reject_unknown(&mut self, known: &[&str]) {
        for key in self.object.keys() {
            if !known.contains(&key.as_str()) {
                let message = format!("unknown key \"{}{key}\" (known: {})", self.prefix, known.join(", "));
                self.problems.push(message);
            }
        }
    }

    /// Globs the config must give.
    fn required_globs(&mut self, key: &str) -> Vec<String> {
        let value = self.object.get(key);
        self.globs(key, value, FILE_NEGATION)
    }

    /// Globs the config may give: none when it does not.
    fn optional_globs(&mut self, key: &str) -> Vec<String> {
        match self.object.get(key) {
            None => Vec::new(),
            value => self.globs(key, value, FILE_NEGATION),
        }
    }

    /// An array of globs that each compile, none starting with `!` (`negation` says why
    /// not), or nothing and a problem saying why not.
    fn globs(&mut self, key: &str, value: Option<&Value>, negation: &str) -> Vec<String> {
        let Some(globs) = value.and_then(string_array) else {
            self.problems.push(format!("\"{}{key}\" must be an array of globs", self.prefix));
            return Vec::new();
        };
        if let Err(why) = GlobMatcher::new(&globs) {
            self.problems.push(format!("\"{}{key}\" has a glob that cannot be read: {why}", self.prefix));
            return Vec::new();
        }
        for negated in globs.iter().filter(|glob| is_negation(glob)) {
            self.problems.push(format!("\"{}{key}\" has {negated:?}: {negation}", self.prefix));
        }
        globs
    }

    fn string(&mut self, key: &str) -> Option<String> {
        match self.object.get(key)? {
            Value::String(s) if !trim(s).is_empty() => Some(s.clone()),
            _ => {
                self.problems.push(format!("\"{}{key}\" must be a non-empty string", self.prefix));
                None
            }
        }
    }

    fn positive_integer(&mut self, key: &str) -> Option<u64> {
        match self.object.get(key)? {
            Value::Number(n) if n.as_u64().is_some_and(|n| n >= 1) => n.as_u64(),
            _ => {
                self.problems.push(format!("\"{}{key}\" must be a positive integer", self.prefix));
                None
            }
        }
    }

    fn nested<T>(&mut self, key: &str, read: impl FnOnce(&mut Fields) -> T) -> Option<T> {
        let value = self.object.get(key)?;
        let Value::Object(object) = value else {
            self.problems.push(format!("\"{}{key}\" must be an object", self.prefix));
            return None;
        };
        let mut reader = Fields { object, prefix: format!("{}{key}.", self.prefix), problems: Vec::new() };
        let result = read(&mut reader);
        self.problems.append(&mut reader.problems);
        Some(result)
    }
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    let Value::Array(items) = value else { return None };
    items
        .iter()
        .map(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn read_config(fields: &mut Fields) -> ProjectConfig {
    let pages = fields.required_globs("pages");
    let frozen = fields.optional_globs("frozen");
    let sources = fields.optional_globs("sources");
    let surfaces = read_surfaces(fields);
    let setup = fields.string("setup");
    let (snapshots_dir, max_age_days) = read_snapshots(fields);
    let adapter = read_adapter(fields);
    let runtime = fields.nested("runtime", |r| {
        r.reject_unknown(&["timeoutMs", "workers"]);
        (r.positive_integer("timeoutMs"), r.positive_integer("workers"))
    });
    let ignore_tags = read_ignore_tags(fields);
    let members = members::read_members(fields);
    let commands = commands::read_commands(fields);
    let undocumented = undocumented::read_undocumented(fields, &surfaces);
    let (timeout_ms, workers) = runtime.unwrap_or((None, None));
    ProjectConfig {
        pages,
        frozen,
        sources,
        surfaces,
        setup,
        snapshots_dir,
        max_age_days,
        adapter,
        timeout_ms: timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
        workers: workers.unwrap_or_else(available_parallelism),
        ignore_tags,
        members,
        commands,
        undocumented,
    }
}

/// How many programs the machine can run at once.
fn available_parallelism() -> u64 {
    std::thread::available_parallelism().map_or(1, |n| n.get() as u64)
}

fn read_surfaces(fields: &mut Fields) -> Vec<(String, SurfaceEntry)> {
    let Some(Value::Object(value)) = fields.object.get("surfaces") else {
        fields.problems.push("\"surfaces\" must name a \"default\" surface".into());
        return Vec::new();
    };
    if value.get(DEFAULT_SURFACE).is_none() {
        fields.problems.push("\"surfaces\" must name a \"default\" surface".into());
        return Vec::new();
    }
    let mut out = Vec::new();
    for (name, entry) in value.iter() {
        match surface_entry(entry) {
            Some(surface) => out.push((name.to_string(), surface)),
            None => fields.problems.push(format!("\"surfaces.{name}\" must be {SHAPE}")),
        }
    }
    out
}

fn surface_entry(entry: &Value) -> Option<SurfaceEntry> {
    match entry {
        Value::String(s) => (!trim(s).is_empty()).then(|| SurfaceEntry::Specifier(s.clone())),
        Value::Object(o) => {
            let mut keys: Vec<&str> = o.keys().map(String::as_str).collect();
            keys.sort_by(|a, b| crate::text::compare_utf16(a, b));
            if keys != ["import", "path"] {
                return None;
            }
            match (o.get("import"), o.get("path")) {
                (Some(Value::String(import)), Some(Value::String(path)))
                    if !import.is_empty() && !path.is_empty() =>
                {
                    Some(SurfaceEntry::Both { import: import.clone(), path: path.clone() })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn read_snapshots(fields: &mut Fields) -> (Option<String>, u64) {
    let snapshots = fields.nested("snapshots", |r| {
        r.reject_unknown(&["dir", "maxAgeDays"]);
        if r.object.get("dir").is_none() {
            r.problems.push("\"snapshots.dir\" is required".into());
        }
        (r.string("dir"), r.positive_integer("maxAgeDays"))
    });
    let (dir, age) = snapshots.unwrap_or((None, None));
    (dir, age.unwrap_or(DEFAULT_SNAPSHOT_MAX_AGE_DAYS))
}

/// Any name is accepted (`/^[a-z][a-z0-9_]*$/`): whether such an adapter exists is found
/// out when it is started. The shape keeps a name usable in a program name and in an
/// environment variable.
fn read_adapter(fields: &mut Fields) -> String {
    let adapter = fields.object.get("adapter");
    if let Some(Value::String(name)) = adapter
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return name.clone();
    }
    let got = adapter.map_or_else(|| "nothing".to_string(), Value::to_string);
    let shape = "such as \"typescript\", in lower-case letters, digits and \"_\"";
    fields.problems.push(format!("\"adapter\" must name a language adapter, {shape} (got {got})"));
    String::new()
}

fn read_ignore_tags(fields: &mut Fields) -> Vec<String> {
    let Some(tags) = fields.object.get("ignoreTags") else {
        return Vec::new();
    };
    let words = string_array(tags).filter(|tags| {
        tags.iter().all(|t| !t.is_empty() && t.chars().all(|c| crate::text::is_word(c) || c == '-'))
    });
    let Some(tags) = words else {
        fields.problems.push("\"ignoreTags\" must be an array of single words".into());
        return Vec::new();
    };
    if tags.iter().any(|t| RESERVED_TAGS.contains(&t.as_str())) {
        fields.problems.push("\"ignoreTags\" cannot contain check, compile or network".into());
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(text: &str) -> String {
        match parse_config(text) {
            Err(Error::User(m)) => m,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn valid_config_gets_defaults() {
        let c = parse_config(
            r#"{"pages":["docs/**/*.md"],"surfaces":{"default":"./src/index.ts"},"adapter":"typescript"}"#,
        )
        .unwrap();
        assert_eq!(c.timeout_ms, 10_000);
        assert_eq!(c.max_age_days, 90);
        assert_eq!(c.surfaces, vec![("default".into(), SurfaceEntry::Specifier("./src/index.ts".into()))]);
    }

    #[test]
    fn a_glob_that_cannot_be_read_or_that_negates_is_refused_with_the_rest() {
        let said = problem(
            r#"{"pages":["docs/*.md",""],"frozen":["./"],"sources":["a/*","!a/b.ts","!(c)"],"surfaces":{"default":"x"},"adapter":"x"}"#,
        );
        assert!(said.contains("\"pages\" has a glob that cannot be read: "), "{said}");
        assert!(said.contains("\"frozen\" has a glob that cannot be read: "), "{said}");
        let negated = "\"sources\" has \"!a/b.ts\": a glob starting with \"!\" matches every file but the ones it names, so it cannot leave files out; \"frozen\" leaves pages out of the check";
        assert!(said.ends_with(negated), "{said}");
    }

    #[test]
    fn every_problem_at_once_in_order() {
        assert_eq!(
            problem(r#"{"pagez":[],"surfaces":{},"adapter":"Python 3","runtime":{"timeoutMs":0}}"#),
            "docattest: invalid docattest.config.json: unknown key \"pagez\" (known: $schema, pages, frozen, sources, surfaces, setup, snapshots, adapter, runtime, ignoreTags, members, commands, undocumented); \"pages\" must be an array of globs; \"surfaces\" must name a \"default\" surface; \"adapter\" must name a language adapter, such as \"typescript\", in lower-case letters, digits and \"_\" (got \"Python 3\"); \"runtime.timeoutMs\" must be a positive integer"
        );
        assert!(problem("[]").ends_with("must contain a JSON object"));
        assert!(problem("{").starts_with("docattest: docattest.config.json is not valid JSON: "));
        assert!(problem(r#"{"pages":[],"surfaces":{"default":"x"}}"#).ends_with("(got nothing)"));
        // Any name of the right shape is a valid config: which adapters exist is not its to know.
        let named = |adapter: &str| {
            parse_config(&format!(r#"{{"pages":[],"surfaces":{{"default":"x"}},"adapter":{adapter}}}"#))
        };
        assert_eq!(named("\"python\"").unwrap().adapter, "python");
        assert_eq!(named("\"c_sharp9\"").unwrap().adapter, "c_sharp9");
        for bad in ["\"\"", "\"a-b\"", "\"../x\"", "\"9a\"", "\"py\\n\"", "3"] {
            assert!(named(bad).unwrap_err().to_string().contains("\"adapter\" must name a language adapter"));
        }
        assert!(
            problem(
                r#"{"pages":[],"surfaces":{"default":"x"},"adapter":"typescript","ignoreTags":["check"]}"#
            )
            .ends_with("\"ignoreTags\" cannot contain check, compile or network")
        );
    }
}
