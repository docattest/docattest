//! Detecting monorepo workspace packages (`pnpm-workspace.yaml` and `package.json#workspaces`)
//! so `docattest init` can discover workspace package READMEs and surfaces automatically.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use yaml_rust2::{Yaml, YamlLoader};

use crate::files::glob::GlobMatcher;

/// One surface discovered during `init`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedSurface {
    pub name: String,
    pub import: Option<String>,
    pub path: String,
}

impl DetectedSurface {
    pub fn plain(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self { name: name.into(), import: None, path: path.into() }
    }

    pub fn to_json(&self) -> Value {
        match &self.import {
            Some(import) => json!({ "import": import, "path": self.path }),
            None => json!(self.path),
        }
    }

    pub fn display_entry(&self) -> String {
        match &self.import {
            Some(import) => format!("{import} ({})", self.path),
            None => self.path.clone(),
        }
    }
}

/// Discover workspace README page globs and package surfaces in a JS/TS monorepo.
pub fn detect_workspace(root: &Path, listed: &[String]) -> (Vec<String>, Vec<DetectedSurface>) {
    let patterns = workspace_patterns(root);
    if patterns.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let dirs = workspace_package_dirs(&patterns, listed);
    let pages = workspace_readme_globs(&patterns, listed);
    let surfaces = workspace_surfaces(root, &dirs, listed);
    (pages, surfaces)
}

fn workspace_patterns(root: &Path) -> Vec<String> {
    let mut patterns = Vec::new();
    if let Ok(text) = fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        patterns.extend(pnpm_patterns(&text));
    }
    if let Ok(text) = fs::read_to_string(root.join("package.json"))
        && let Ok(pkg) = serde_json::from_str::<Value>(&text)
    {
        patterns.extend(package_json_patterns(&pkg));
    }
    patterns
}

fn pnpm_patterns(text: &str) -> Vec<String> {
    let Ok(docs) = YamlLoader::load_from_str(text) else { return Vec::new() };
    let Some(Yaml::Hash(map)) = docs.first() else { return Vec::new() };
    let Some(Yaml::Array(items)) = map.get(&Yaml::String("packages".into())) else {
        return Vec::new();
    };
    items.iter().filter_map(Yaml::as_str).map(trim_slash).collect()
}

fn package_json_patterns(pkg: &Value) -> Vec<String> {
    let list = match pkg.get("workspaces") {
        Some(Value::Array(arr)) => Some(arr),
        Some(Value::Object(obj)) => obj.get("packages").and_then(Value::as_array),
        _ => None,
    };
    list.into_iter().flatten().filter_map(Value::as_str).map(trim_slash).collect()
}

fn trim_slash(pattern: &str) -> String {
    pattern.trim_end_matches('/').to_string()
}

fn workspace_package_dirs(patterns: &[String], listed: &[String]) -> Vec<String> {
    let includes: Vec<String> =
        patterns.iter().filter(|p| !p.starts_with('!')).map(|p| format!("{p}/package.json")).collect();
    let excludes: Vec<String> =
        patterns.iter().filter_map(|p| p.strip_prefix('!')).map(|p| format!("{p}/package.json")).collect();
    let Ok(include) = GlobMatcher::new(&includes) else { return Vec::new() };
    let exclude = GlobMatcher::new(&excludes).ok();
    let mut dirs = BTreeSet::new();
    for file in listed {
        if include.is_match(file)
            && !exclude.as_ref().is_some_and(|m| m.is_match(file))
            && let Some(dir) = file.strip_suffix("/package.json")
        {
            dirs.insert(dir.to_string());
        }
    }
    dirs.into_iter().collect()
}

fn workspace_readme_globs(patterns: &[String], listed: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for pat in patterns.iter().filter(|p| !p.starts_with('!')) {
        let glob = format!("{pat}/README.md");
        if !out.contains(&glob)
            && GlobMatcher::new(std::slice::from_ref(&glob))
                .is_ok_and(|m| listed.iter().any(|f| m.is_match(f)))
        {
            out.push(glob);
        }
    }
    out
}

fn workspace_surfaces(root: &Path, dirs: &[String], listed: &[String]) -> Vec<DetectedSurface> {
    let mut surfaces = Vec::new();
    let mut used_names = BTreeSet::new();
    for dir in dirs {
        let Some(pkg) = read_package_json(&root.join(dir).join("package.json")) else { continue };
        let Some(entry) = package_entry(root, dir, &pkg, listed) else { continue };
        let Some(pkg_name) = pkg.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            continue;
        };
        let name = unique_surface_name(pkg_name, dir, &mut used_names);
        surfaces.push(DetectedSurface { name, import: Some(pkg_name.to_string()), path: entry });
    }
    if surfaces.len() == 1 {
        surfaces[0].name = "default".into();
    }
    surfaces
}

fn read_package_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn package_entry(root: &Path, dir: &str, pkg: &Value, listed: &[String]) -> Option<String> {
    if has_compiler_transform(&root.join(dir))
        && let Some(built) = built_entry(root, dir, pkg)
    {
        return Some(built);
    }
    for candidate in ["src/index.ts", "src/index.tsx", "src/index.js", "index.ts", "index.js"] {
        let rel = format!("{dir}/{candidate}");
        if listed.iter().any(|f| f == &rel) {
            return Some(format!("./{rel}"));
        }
    }
    built_entry(root, dir, pkg)
}

fn built_entry(root: &Path, dir: &str, pkg: &Value) -> Option<String> {
    for key in ["main", "module"] {
        if let Some(raw) = pkg.get(key).and_then(Value::as_str) {
            let clean = raw.trim_start_matches("./");
            let rel = if dir.is_empty() { clean.to_string() } else { format!("{dir}/{clean}") };
            if root.join(&rel).is_file() {
                return Some(format!("./{rel}"));
            }
        }
    }
    None
}

/// Whether `dir/tsconfig.json` declares a custom TypeScript compiler transform plugin
/// (`compilerOptions.plugins[].transform`), meaning raw `.ts` sources cannot run without a build.
pub(super) fn has_compiler_transform(dir: &Path) -> bool {
    let Ok(text) = fs::read_to_string(dir.join("tsconfig.json")) else { return false };
    let Ok(tsconfig) = serde_json::from_str::<Value>(&text) else { return false };
    tsconfig
        .get("compilerOptions")
        .and_then(|o| o.get("plugins"))
        .and_then(Value::as_array)
        .is_some_and(|plugins| plugins.iter().any(|p| p.get("transform").is_some()))
}

fn unique_surface_name(pkg_name: &str, dir: &str, used: &mut BTreeSet<String>) -> String {
    let short = pkg_name.rsplit('/').next().unwrap_or(pkg_name);
    let base = sanitize_surface_name(short);
    if !base.is_empty() && used.insert(base.clone()) {
        return base;
    }
    let fallback = sanitize_surface_name(dir);
    if !fallback.is_empty() && used.insert(fallback.clone()) {
        return fallback;
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{base}_{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

fn sanitize_surface_name(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
