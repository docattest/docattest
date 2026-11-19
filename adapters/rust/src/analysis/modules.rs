//! A crate's modules, read from syntax and never compiled: which files they are, what
//! they declare, and what they export.

use super::docs_helpers::is_docs_helper;
use crate::language::doc::named;
use std::path::{Path, PathBuf};
use syn::{Item, UseTree, Visibility};

/// Reads a file; `None` when there is none. A revision of a project reads its own.
pub type Read<'a> = &'a dyn Fn(&Path) -> Option<String>;

/// One module of a crate.
pub struct Module {
    /// Its path below the crate's root: empty for the root itself.
    pub path: Vec<String>,
    /// The file it is written in.
    pub file: PathBuf,
    /// Where its child modules' files are looked for.
    dir: PathBuf,
    pub items: Vec<Item>,
}

/// A crate's library, as its modules.
pub struct Crate {
    /// The name the crate is imported by.
    pub name: String,
    pub modules: Vec<Module>,
}

/// A name a module exports.
#[derive(Clone, Debug, PartialEq)]
pub struct Export {
    pub name: String,
    /// The module and name of the declaration it refers to, when that is in the crate.
    pub origin: Option<(Vec<String>, String)>,
}

/// Whether an item is `pub`, with no restriction.
pub(super) fn is_public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

/// The visibility of an item that has a name.
pub(super) fn visibility(item: &Item) -> Option<&Visibility> {
    Some(match item {
        Item::Const(i) => &i.vis,
        Item::Static(i) => &i.vis,
        Item::Fn(i) => &i.vis,
        Item::Struct(i) => &i.vis,
        Item::Enum(i) => &i.vis,
        Item::Union(i) => &i.vis,
        Item::Trait(i) => &i.vis,
        Item::Type(i) => &i.vis,
        Item::Mod(i) => &i.vis,
        _ => return None,
    })
}

/// The leaves of a `use` tree: the name each brings in (`*` for a glob) and the path it
/// stands for.
pub fn use_leaves(tree: &UseTree) -> Vec<(String, Vec<String>)> {
    fn walk(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<(String, Vec<String>)>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                walk(&path.tree, prefix, out);
                prefix.pop();
            }
            UseTree::Name(name) if name.ident == "self" => {
                out.push((prefix.last().cloned().unwrap_or_default(), prefix.clone()));
            }
            UseTree::Name(name) => out.push((name.ident.to_string(), with(prefix, name.ident.to_string()))),
            UseTree::Rename(rename) => {
                out.push((rename.rename.to_string(), with(prefix, rename.ident.to_string())))
            }
            UseTree::Glob(_) => out.push(("*".into(), prefix.clone())),
            UseTree::Group(group) => group.items.iter().for_each(|inner| walk(inner, prefix, out)),
        }
    }
    fn with(prefix: &[String], last: String) -> Vec<String> {
        prefix.iter().cloned().chain([last]).collect()
    }
    let mut out = Vec::new();
    walk(tree, &mut Vec::new(), &mut out);
    out
}

impl Crate {
    /// Reads a crate from its root file, following its `mod` declarations to their files.
    pub fn load(name: &str, root: &Path, read: Read) -> Crate {
        let mut krate = Crate { name: name.to_string(), modules: Vec::new() };
        let dir = root.parent().unwrap_or(Path::new("")).to_path_buf();
        if let Some(file) = read(root).and_then(|text| syn::parse_file(&text).ok()) {
            krate.add(Module { path: Vec::new(), file: root.to_path_buf(), dir, items: file.items }, read);
        }
        krate
    }

    /// Adds a module, and then the modules it declares.
    fn add(&mut self, module: Module, read: Read) {
        let children: Vec<Module> =
            module.items.iter().filter_map(|item| child(&module, item, read)).collect();
        self.modules.push(module);
        for found in children {
            self.add(found, read);
        }
    }

    /// The module at a path below the root.
    pub fn module(&self, path: &[String]) -> Option<&Module> {
        self.modules.iter().find(|module| module.path == path)
    }

    /// Resolves a path written in a module to the module and name it stands for, as far
    /// as syntax tells: through `crate`, `self`, `super`, the crate's own name, and the
    /// modules below the one it is written in.
    pub fn resolve(&self, from: &[String], written: &[String]) -> Option<(Vec<String>, String)> {
        let (name, prefix) = written.split_last()?;
        let mut at = from.to_vec();
        for (k, segment) in prefix.iter().enumerate() {
            match segment.as_str() {
                "crate" => at.clear(),
                "self" => {}
                "super" => {
                    at.pop()?;
                }
                own if k == 0 && own == self.name => at.clear(),
                below => at.push(below.to_string()),
            }
        }
        self.module(&at).map(|_| (at, name.clone()))
    }

    /// Whether the module at a path is declared `pub`, with no restriction, by its parent;
    /// the root is.
    pub fn declared_public(&self, path: &[String]) -> bool {
        let Some((name, parent)) = path.split_last() else { return true };
        let declares = |item: &&Item| matches!(item, Item::Mod(declared) if declared.ident == name);
        let declaration = self.module(parent).and_then(|parent| parent.items.iter().find(declares));
        declaration.and_then(visibility).is_some_and(is_public)
    }

    /// The names a module exports: its own `pub` items that are neither modules nor docs
    /// helpers, and what it re-exports with `pub use`, a glob's by the exports of the
    /// module it names.
    pub fn exports(&self, path: &[String]) -> Vec<Export> {
        self.exports_within(path, &mut Vec::new())
    }

    fn exports_within(&self, path: &[String], seen: &mut Vec<Vec<String>>) -> Vec<Export> {
        let Some(module) = self.module(path).filter(|_| !seen.iter().any(|s| s == path)) else {
            return Vec::new();
        };
        seen.push(path.to_vec());
        let mut out: Vec<Export> = Vec::new();
        for item in &module.items {
            match item {
                Item::Use(used) if is_public(&used.vis) => {
                    for (name, target) in use_leaves(&used.tree) {
                        out.extend(self.reexport(path, name, &target, seen));
                    }
                }
                Item::Mod(_) => {}
                helper if is_docs_helper(helper) => {}
                other => {
                    if let Some((name, _)) = named(other).filter(|_| visibility(other).is_some_and(is_public))
                    {
                        out.push(Export { origin: Some((path.to_vec(), name.clone())), name });
                    }
                }
            }
        }
        out.dedup_by(|a, b| a.name == b.name);
        out
    }

    /// What one leaf of a `pub use` exports.
    fn reexport(
        &self,
        from: &[String],
        name: String,
        target: &[String],
        seen: &mut Vec<Vec<String>>,
    ) -> Vec<Export> {
        if name != "*" {
            let origin = self.resolve(from, target).map(|(module, item)| self.origin(&module, &item));
            return vec![Export { name, origin }];
        }
        self.whole(from, target).map(|module| self.exports_within(&module, seen)).unwrap_or_default()
    }

    /// The module of the crate a glob names, written in a module.
    pub fn whole(&self, from: &[String], target: &[String]) -> Option<Vec<String>> {
        let any: Vec<String> = target.iter().cloned().chain([String::new()]).collect();
        self.resolve(from, &any).map(|(module, _)| module)
    }

    /// Whether syntax cannot say everything a module exports: a macro is called where
    /// its items are, or it re-exports whole something that is not a module of this
    /// crate, or a module syntax cannot say everything of.
    pub fn opaque(&self, path: &[String]) -> bool {
        self.opaque_within(path, &mut Vec::new())
    }

    fn opaque_within(&self, path: &[String], seen: &mut Vec<Vec<String>>) -> bool {
        let Some(module) = self.module(path).filter(|_| !seen.iter().any(|s| s == path)) else {
            return false;
        };
        seen.push(path.to_vec());
        module.items.iter().any(|item| match item {
            Item::Use(used) if is_public(&used.vis) => {
                let leaves = use_leaves(&used.tree);
                let mut globs = leaves.iter().filter(|(name, _)| name == "*");
                globs.any(|(_, target)| match self.whole(path, target) {
                    Some(module) => self.opaque_within(&module, seen),
                    None => true,
                })
            }
            other => calls_macro(other),
        })
    }

    /// The declaration a name of a module refers to, through its re-exports.
    pub fn origin(&self, module: &[String], name: &str) -> (Vec<String>, String) {
        let found = self
            .exports(module)
            .into_iter()
            .find(|export| export.name == name)
            .and_then(|export| export.origin);
        found
            .filter(|origin| origin.0 != module || origin.1 != name)
            .map_or_else(|| (module.to_vec(), name.to_string()), |(module, name)| self.origin(&module, &name))
    }
}

/// Whether an item is a macro call, which may write any number of items, or something
/// the parser could not read as an item.
pub fn calls_macro(item: &Item) -> bool {
    match item {
        Item::Macro(call) => call.ident.is_none(),
        Item::Verbatim(_) => true,
        _ => false,
    }
}

/// The module an item declares, when it declares one: written in place, or in a file.
fn child(parent: &Module, item: &Item, read: Read) -> Option<Module> {
    let Item::Mod(declared) = item else { return None };
    let name = declared.ident.to_string();
    let path: Vec<String> = parent.path.iter().cloned().chain([name.clone()]).collect();
    if let Some((_, items)) = &declared.content {
        return Some(Module {
            path,
            file: parent.file.clone(),
            dir: parent.dir.join(&name),
            items: items.clone(),
        });
    }
    let candidates = [parent.dir.join(format!("{name}.rs")), parent.dir.join(&name).join("mod.rs")];
    let (file, text) = candidates.into_iter().find_map(|file| read(&file).map(|text| (file, text)))?;
    let items = syn::parse_file(&text).ok()?.items;
    Some(Module { path, file, dir: parent.dir.join(&name), items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn krate() -> Crate {
        let files: HashMap<&str, &str> = HashMap::from([
            (
                "/c/src/lib.rs",
                "mod retry;\npub mod deep;\npub use retry::{retry, RETRY as DEFAULTS};\npub use deep::*;\npub const N: u8 = 1;\nfn private() {}\npub(crate) fn inner() {}\npub mod here { pub fn inline() {} }\n#[cfg(docattest)]\npub fn for_pages() {}",
            ),
            ("/c/src/retry.rs", "pub fn retry() {}\npub const RETRY: u8 = 3;"),
            ("/c/src/deep/mod.rs", "mod below;\npub use below::Low;\npub fn deep() {}"),
            ("/c/src/deep/below.rs", "pub struct Low;\nimpl Low { pub fn new() -> Low { Low } }"),
        ]);
        Crate::load("c", Path::new("/c/src/lib.rs"), &|path| {
            files.get(path.to_str()?).map(|text| text.to_string())
        })
    }

    fn path(written: &str) -> Vec<String> {
        written.split("::").filter(|segment| !segment.is_empty()).map(str::to_string).collect()
    }

    #[test]
    fn modules_are_found_through_their_declarations() {
        let krate = krate();
        let found: Vec<String> =
            krate.modules.iter().map(|m| format!("{}={}", m.path.join("::"), m.file.display())).collect();
        assert_eq!(
            found,
            [
                "=/c/src/lib.rs",
                "retry=/c/src/retry.rs",
                "deep=/c/src/deep/mod.rs",
                "deep::below=/c/src/deep/below.rs",
                "here=/c/src/lib.rs"
            ]
        );
    }

    #[test]
    fn exports_are_public_items_and_what_is_reexported() {
        let krate = krate();
        let names =
            |module: &str| krate.exports(&path(module)).into_iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(names(""), ["retry", "DEFAULTS", "Low", "deep", "N"]);
        assert_eq!(names("deep"), ["Low", "deep"]);
        assert_eq!(names("here"), ["inline"]);
        assert_eq!(krate.origin(&[], "DEFAULTS"), (path("retry"), "RETRY".to_string()));
        assert_eq!(krate.origin(&[], "Low"), (path("deep::below"), "Low".to_string()));
        assert_eq!(krate.origin(&[], "N"), (Vec::new(), "N".to_string()));
    }

    #[test]
    fn modules_are_public_when_their_parent_declares_them_pub() {
        let krate = krate();
        let public: Vec<bool> = ["", "deep", "here", "retry", "deep::below"]
            .map(|module| krate.declared_public(&path(module)))
            .to_vec();
        assert_eq!(public, [true, true, true, false, false]);
    }

    #[test]
    fn a_module_syntax_cannot_list_is_told_from_one_it_can() {
        let opaque = |root: &str| {
            let read = |path: &Path| match path.to_str()? {
                "/c/src/lib.rs" => Some(root.to_string()),
                "/c/src/made.rs" => Some("make!(written);".to_string()),
                _ => None,
            };
            Crate::load("c", Path::new("/c/src/lib.rs"), &read).opaque(&[])
        };
        assert!(!opaque(
            "macro_rules! make { () => {}; }\npub fn plain() {}\nmod made;\npub use made::written;"
        ));
        assert!(opaque("thread_local! { pub static N: u8 = 1; }"));
        assert!(opaque("mod made;\npub use made::*;"));
        assert!(opaque("pub use another_crate::*;"));
        assert!(opaque("pub enum Kind { Small }\npub use Kind::*;"));
    }

    #[test]
    fn paths_resolve_through_crate_self_super_and_the_crates_name() {
        let krate = krate();
        let from = path("deep::below");
        for written in ["crate::retry::retry", "c::retry::retry", "super::super::retry::retry"] {
            assert_eq!(
                krate.resolve(&from, &path(written)),
                Some((path("retry"), "retry".to_string())),
                "{written}"
            );
        }
        assert_eq!(krate.resolve(&from, &path("self::Low")), Some((from.clone(), "Low".to_string())));
        assert_eq!(krate.resolve(&from, &path("nowhere::x")), None);
    }
}
