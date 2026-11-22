//! Detecting a project's language adapter, docs pages and default surface for `init`.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::config::NONE_ADAPTER;
use crate::files::project_files::ProjectFiles;
use crate::files::source::DiskFiles;
use crate::git::ignored::ignored_by_git;

/// Published JSON Schema URI for `docattest.config.json`.
pub const CONFIG_SCHEMA_URL: &str = "https://docattest.github.io/docattest/schema/config.json";

/// Detected project configuration before writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedConfig {
    pub adapter: String,
    pub pages: Vec<String>,
    pub surface: Option<String>,
}

impl DetectedConfig {
    pub fn to_config_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("$schema".into(), json!(CONFIG_SCHEMA_URL));
        obj.insert("pages".into(), json!(self.pages));
        obj.insert("adapter".into(), json!(self.adapter));
        if let Some(surface) = &self.surface {
            obj.insert("surfaces".into(), json!({ "default": surface }));
        }
        Value::Object(obj)
    }
}

/// Inspect `root` and detect its adapter, Markdown/MDX page globs, and default surface.
pub fn detect(root: &Path) -> DetectedConfig {
    let mut files = DiskFiles::new(root.to_path_buf());
    files.ignore_what_git_ignores(ignored_by_git(root));
    let listed = files.list();
    let adapter = detect_adapter(root, &listed);
    let pages = detect_pages(&listed);
    let surface = detect_surface(root, &adapter, &listed);
    DetectedConfig { adapter, pages, surface }
}

fn detect_adapter(root: &Path, listed: &[String]) -> String {
    if prefers_typescript(root, listed) {
        return "typescript".into();
    }
    const MARKERS: [(&str, &str); 5] = [
        ("pyproject.toml", "python"),
        ("setup.py", "python"),
        ("setup.cfg", "python"),
        ("go.mod", "go"),
        ("Cargo.toml", "rust"),
    ];
    for (file, adapter) in MARKERS {
        if root.join(file).is_file() {
            return adapter.into();
        }
    }
    NONE_ADAPTER.into()
}

fn prefers_typescript(root: &Path, listed: &[String]) -> bool {
    if !root.join("package.json").is_file() {
        return false;
    }
    let has_ts_or_js = root.join("tsconfig.json").is_file()
        || listed.iter().any(|f| f.starts_with("src/") && (f.ends_with(".ts") || f.ends_with(".js")));
    let other_lang = ["Cargo.toml", "go.mod", "pyproject.toml"].iter().any(|f| root.join(f).is_file());
    has_ts_or_js || !other_lang
}

fn detect_pages(listed: &[String]) -> Vec<String> {
    let mut pages = Vec::new();
    if listed.iter().any(|f| f == "README.md") {
        pages.push("README.md".into());
    }
    for dir in ["docs", "doc", "documentation", "guide"] {
        let found = dir_page_globs(listed, dir);
        let found_docs = dir == "docs" && !found.is_empty();
        pages.extend(found);
        if found_docs {
            break;
        }
    }
    if pages.is_empty() {
        return vec!["README.md".into(), "docs/**/*.md".into()];
    }
    pages
}

fn dir_page_globs(listed: &[String], dir: &str) -> Vec<String> {
    let prefix = format!("{dir}/");
    let mut out = Vec::new();
    for ext in [".md", ".mdx"] {
        if listed.iter().any(|f| f.starts_with(&prefix) && f.ends_with(ext)) {
            out.push(format!("{dir}/**/*{ext}"));
        }
    }
    out
}

fn detect_surface(root: &Path, adapter: &str, listed: &[String]) -> Option<String> {
    match adapter {
        "typescript" => detect_ts_surface(root, listed),
        "python" => detect_python_surface(root, listed),
        "go" => detect_go_surface(listed),
        "rust" => detect_rust_surface(root),
        _ => None,
    }
}

fn detect_ts_surface(root: &Path, listed: &[String]) -> Option<String> {
    for candidate in ["src/index.ts", "src/index.tsx", "src/index.js", "index.ts", "index.js"] {
        if listed.iter().any(|f| f == candidate) {
            return Some(format!("./{candidate}"));
        }
    }
    let pkg: Value = serde_json::from_str(&fs::read_to_string(root.join("package.json")).ok()?).ok()?;
    for key in ["main", "module"] {
        if let Some(entry) = pkg.get(key).and_then(Value::as_str) {
            let clean = entry.trim_start_matches("./");
            if root.join(clean).is_file() {
                return Some(format!("./{clean}"));
            }
        }
    }
    None
}

fn detect_python_surface(root: &Path, listed: &[String]) -> Option<String> {
    if let Ok(text) = fs::read_to_string(root.join("pyproject.toml"))
        && let Ok(parsed) = toml::from_str::<toml::Value>(&text)
        && let Some(name) = parsed.get("project").and_then(|p| p.get("name")).and_then(toml::Value::as_str)
    {
        let module = name.replace('-', "_");
        let in_src = format!("src/{module}/__init__.py");
        let in_root = format!("{module}/__init__.py");
        if listed.iter().any(|f| f == &in_src || f == &in_root) {
            return Some(module);
        }
    }
    for file in listed {
        if let Some(rest) = file.strip_prefix("src/").and_then(|s| s.strip_suffix("/__init__.py"))
            && !rest.contains('/')
        {
            return Some(rest.to_string());
        }
    }
    None
}

fn detect_go_surface(listed: &[String]) -> Option<String> {
    let has_root_go =
        listed.iter().any(|f| !f.contains('/') && f.ends_with(".go") && !f.ends_with("_test.go"));
    if has_root_go { Some(".".into()) } else { None }
}

fn detect_rust_surface(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let parsed: toml::Value = toml::from_str(&text).ok()?;
    if root.join("src/lib.rs").is_file() {
        let lib_name = parsed
            .get("lib")
            .and_then(|l| l.get("name"))
            .and_then(toml::Value::as_str)
            .or_else(|| parsed.get("package").and_then(|p| p.get("name")).and_then(toml::Value::as_str));
        return lib_name.map(|n| n.replace('-', "_"));
    }
    if root.join("src/main.rs").is_file() {
        return Some("./src/main.rs".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Temp;

    #[test]
    fn detects_typescript_with_mdx_and_surface() {
        let tmp = Temp::new("init-ts");
        tmp.write("package.json", r#"{"name":"demo"}"#);
        tmp.write("tsconfig.json", "{}");
        tmp.write("src/index.ts", "export const N = 1;");
        tmp.write("README.md", "# Demo\n");
        tmp.write("docs/guide.mdx", "# Guide\n");
        let detected = detect(&tmp.0);
        assert_eq!(detected.adapter, "typescript");
        assert_eq!(detected.pages, vec!["README.md", "docs/**/*.mdx"]);
        assert_eq!(detected.surface.as_deref(), Some("./src/index.ts"));
    }

    #[test]
    fn detects_core_only_repo_without_language_or_surface() {
        let tmp = Temp::new("init-none");
        tmp.write("README.md", "# Ops\n");
        tmp.write("Dockerfile", "FROM scratch\n");
        let detected = detect(&tmp.0);
        assert_eq!(detected.adapter, "none");
        assert_eq!(detected.pages, vec!["README.md"]);
        assert_eq!(detected.surface, None);
    }

    #[test]
    fn detects_rust_crate_lib_and_python_package() {
        let rust = Temp::new("init-rust");
        rust.write("Cargo.toml", "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n");
        rust.write("src/lib.rs", "pub const N: u32 = 1;\n");
        rust.write("README.md", "# Crate\n");
        let detected = detect(&rust.0);
        assert_eq!(detected.adapter, "rust");
        assert_eq!(detected.surface.as_deref(), Some("my_crate"));

        let py = Temp::new("init-py");
        py.write("pyproject.toml", "[project]\nname = \"my-pkg\"\n");
        py.write("src/my_pkg/__init__.py", "N = 1\n");
        py.write("README.md", "# Pkg\n");
        let detected = detect(&py.0);
        assert_eq!(detected.adapter, "python");
        assert_eq!(detected.surface.as_deref(), Some("my_pkg"));
    }
}
