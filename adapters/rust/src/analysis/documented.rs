//! A crate's library as the compiler lists it, read from rustdoc's JSON: what each module
//! exports once every macro has written what it writes. Syntax cannot say that, so the
//! compiler is asked about a module whose source does not declare all it exports, and
//! about no other.

use super::Analysis;
use super::modules::Export;
use crate::project::Package;
use crate::toolchain::rustdoc::{self, Library};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// How a module's exports are known.
pub(crate) enum Listing {
    /// Its source declares them all.
    Read,
    /// The compiler lists them.
    Compiled(Arc<Documented>),
    /// Its source does not declare them all, and this is why the compiler could not
    /// list them.
    Refused(String),
}

impl Analysis {
    /// How the exports of a module of a package's library are known. The compiler is
    /// asked once for a library, in a build directory of its own, so that it does not
    /// wait for the build of the examples.
    pub(super) fn listing(&self, package: &Package, module: &[String]) -> Listing {
        if !self.krate(package).opaque(module) {
            return Listing::Read;
        }
        let mut asked = self.documented.lock().unwrap_or_else(|p| p.into_inner());
        let answer = asked.entry(package.lib.clone()).or_insert_with(|| {
            let library = Library { package: &package.name, name: &package.lib, dir: &package.dir };
            let text = rustdoc::document(&library, &self.project.work.join("doc"))?;
            Documented::read(&text).map(Arc::new)
        });
        match answer {
            Ok(documented) => Listing::Compiled(Arc::clone(documented)),
            Err(why) => Listing::Refused(why.clone()),
        }
    }
}

/// The kinds of item a module exports by name, besides an enum: the ones its syntax
/// counts.
const EXPORTED: [&str; 7] = ["constant", "static", "function", "struct", "union", "trait", "type_alias"];

/// What an item is called in the tables of rustdoc's JSON.
type Id = u32;

/// The items of a documented crate.
#[derive(Deserialize)]
pub(crate) struct Documented {
    root: Id,
    index: HashMap<Id, Item>,
    /// Where each item is declared, for the items other items refer to.
    paths: HashMap<Id, Declared>,
}

#[derive(Deserialize)]
struct Item {
    name: Option<String>,
    visibility: Visibility,
    inner: Inner,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Visibility {
    Public,
    /// As its parent: an enum's variant, a trait's method.
    Default,
    Crate,
    Restricted(IgnoredAny),
}

/// What an item is, as far as its exports are concerned.
#[derive(Deserialize)]
#[serde(try_from = "HashMap<String, Value>")]
enum Inner {
    /// The items a module holds.
    Module(Vec<Id>),
    /// The variants of an enum.
    Enum(Vec<Id>),
    Use(Use),
    /// Any other kind, by rustdoc's name for it.
    Other(String),
}

#[derive(Deserialize)]
struct Use {
    /// The name the item is brought in by; a glob has none of its own.
    name: String,
    /// What is brought in; none when rustdoc does not know the item.
    id: Option<Id>,
    is_glob: bool,
}

#[derive(Deserialize)]
struct Declared {
    /// 0 for the documented crate itself.
    crate_id: u32,
    /// The crate's name, the modules below it, and the item's name.
    path: Vec<String>,
}

impl TryFrom<HashMap<String, Value>> for Inner {
    type Error = String;

    /// Reads `{ "<kind>": { … } }`.
    fn try_from(tagged: HashMap<String, Value>) -> Result<Inner, String> {
        #[derive(Deserialize)]
        struct Module {
            items: Vec<Id>,
        }
        #[derive(Deserialize)]
        struct Enum {
            variants: Vec<Id>,
        }
        let (kind, body) = tagged.into_iter().next().ok_or("an item of no kind")?;
        let unreadable = |err: serde_json::Error| format!("{kind}: {err}");
        Ok(match kind.as_str() {
            "module" => Inner::Module(serde_json::from_value::<Module>(body).map_err(unreadable)?.items),
            "enum" => Inner::Enum(serde_json::from_value::<Enum>(body).map_err(unreadable)?.variants),
            "use" => Inner::Use(serde_json::from_value(body).map_err(unreadable)?),
            _ => Inner::Other(kind),
        })
    }
}

impl Documented {
    /// Reads rustdoc's JSON; an error when it is not in the shape this adapter knows,
    /// as a later Rust may write it.
    pub(crate) fn read(text: &str) -> Result<Documented, String> {
        #[derive(Deserialize)]
        struct Format {
            format_version: u32,
        }
        let unreadable = |why: String| match serde_json::from_str::<Format>(text) {
            Ok(Format { format_version }) => {
                format!("rustdoc's JSON (format {format_version}) is not one this adapter reads: {why}")
            }
            Err(_) => format!("rustdoc's JSON cannot be read: {why}"),
        };
        let documented: Documented = serde_json::from_str(text).map_err(|err| unreadable(err.to_string()))?;
        match documented.index.get(&documented.root).map(|root| &root.inner) {
            Some(Inner::Module(_)) => Ok(documented),
            _ => Err(unreadable("its root is not a module".to_string())),
        }
    }

    /// The names a module exports, by its path below the crate's root; `None` when the
    /// crate has no such module.
    pub(crate) fn exports(&self, path: &[String]) -> Option<Vec<Export>> {
        let mut out = Vec::new();
        self.list(self.module(path)?, &mut Vec::new(), &mut out);
        Some(out)
    }

    /// The items the module at a path holds.
    fn module(&self, path: &[String]) -> Option<&[Id]> {
        let mut items = self.held(self.root)?;
        for name in path {
            items = items.iter().find_map(|id| {
                let named = self.index.get(id)?.name.as_deref() == Some(name.as_str());
                self.held(*id).filter(|_| named)
            })?;
        }
        Some(items)
    }

    /// The items an item holds, when it is a module.
    fn held(&self, id: Id) -> Option<&[Id]> {
        match &self.index.get(&id)?.inner {
            Inner::Module(items) => Some(items),
            _ => None,
        }
    }

    /// Adds what a module's items export: its public items that are not modules, what it
    /// re-exports by name, and what it re-exports whole.
    fn list(&self, items: &[Id], seen: &mut Vec<Id>, out: &mut Vec<Export>) {
        for id in items {
            let Some(item) = self.index.get(id).filter(|item| matches!(item.visibility, Visibility::Public))
            else {
                continue;
            };
            let named =
                |name: &str, declared: Id| Export { name: name.to_string(), origin: self.declared(declared) };
            let export = match (&item.inner, &item.name) {
                (Inner::Use(used), _) if used.is_glob => {
                    self.list_whole(used.id, seen, out);
                    None
                }
                (Inner::Use(used), _) => {
                    Some(Export { name: used.name.clone(), origin: used.id.and_then(|id| self.declared(id)) })
                }
                (Inner::Enum(_), Some(name)) => Some(named(name, *id)),
                (Inner::Other(kind), Some(name)) if EXPORTED.contains(&kind.as_str()) => {
                    Some(named(name, *id))
                }
                _ => None,
            };
            add(out, export);
        }
    }

    /// Adds what a glob brings in: the exports of a module of the crate, or the variants
    /// of one of its enums.
    fn list_whole(&self, whole: Option<Id>, seen: &mut Vec<Id>, out: &mut Vec<Export>) {
        let Some(whole) = whole.filter(|id| !seen.contains(id)) else { return };
        seen.push(whole);
        match self.index.get(&whole).map(|item| &item.inner) {
            Some(Inner::Module(items)) => self.list(items, seen, out),
            Some(Inner::Enum(variants)) => {
                let names = variants.iter().filter_map(|id| self.index.get(id)?.name.clone());
                // A variant is declared by its enum.
                names.for_each(|name| add(out, Some(Export { name, origin: self.declared(whole) })));
            }
            _ => {}
        }
    }

    /// The module and name an item of the crate is declared by.
    fn declared(&self, id: Id) -> Option<(Vec<String>, String)> {
        let place = self.paths.get(&id).filter(|place| place.crate_id == 0)?;
        let (name, module) = place.path.split_last()?;
        Some((module.iter().skip(1).cloned().collect(), name.clone()))
    }
}

/// Adds an export, unless one of its name is there: what a module declares comes
/// before what a glob brings in.
fn add(out: &mut Vec<Export>, export: Option<Export>) {
    out.extend(export.filter(|export| !out.iter().any(|held| held.name == export.name)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A crate `c`: `pub use low::*`, `pub use low::Low as Renamed`, a function a macro
    /// wrote, a private constant, a re-export of another crate's item, and
    /// `pub use Kind::*` of its own enum.
    fn documented() -> Documented {
        let item = |name: Value, visibility: &str, inner: Value| json!({ "name": name, "visibility": visibility, "inner": inner });
        let reexport = |name: &str, id: u32, is_glob: bool| {
            item(Value::Null, "public", json!({ "use": { "name": name, "id": id, "is_glob": is_glob } }))
        };
        let whole = json!({
            "format_version": 61,
            "root": 0,
            "index": {
                "0": item(json!("c"), "public", json!({ "module": { "items": [1, 2, 3, 4, 5, 9, 10, 11] } })),
                "1": item(json!("low"), "crate", json!({ "module": { "items": [6, 7] } })),
                "2": reexport("low", 1, true),
                "3": reexport("Renamed", 6, false),
                "4": item(json!("by_macro"), "public", json!({ "function": {} })),
                "5": item(json!("HIDDEN"), "crate", json!({ "constant": {} })),
                "6": item(json!("Low"), "public", json!({ "struct": {} })),
                "7": item(json!("deep"), "public", json!({ "module": { "items": [8] } })),
                "8": item(json!("below"), "public", json!({ "function": {} })),
                "9": reexport("Map", 20, false),
                "10": item(json!("Kind"), "public", json!({ "enum": { "variants": [12] } })),
                "11": reexport("Kind", 10, true),
                "12": item(json!("Small"), "default", json!({ "variant": {} })),
                "13": item(json!("make"), "public", json!({ "macro": "macro_rules! make" })),
            },
            "paths": {
                "4": { "crate_id": 0, "path": ["c", "by_macro"] },
                "6": { "crate_id": 0, "path": ["c", "low", "Low"] },
                "10": { "crate_id": 0, "path": ["c", "Kind"] },
                "20": { "crate_id": 1, "path": ["std", "collections", "HashMap"] },
            },
        });
        Documented::read(&whole.to_string()).unwrap()
    }

    fn path(written: &str) -> Vec<String> {
        written.split("::").filter(|segment| !segment.is_empty()).map(str::to_string).collect()
    }

    #[test]
    fn exports_are_what_the_compiler_lists() {
        let exports = documented().exports(&[]).unwrap();
        let export = |name: &str, origin: Option<(&str, &str)>| Export {
            name: name.to_string(),
            origin: origin.map(|(module, name)| (path(module), name.to_string())),
        };
        let want = [
            export("Low", Some(("low", "Low"))),
            export("Renamed", Some(("low", "Low"))),
            export("by_macro", Some(("", "by_macro"))),
            export("Map", None),
            export("Kind", Some(("", "Kind"))),
            export("Small", Some(("", "Kind"))),
        ];
        assert_eq!(exports, want);
    }

    #[test]
    fn a_module_is_found_by_its_path() {
        let documented = documented();
        let names = |module: &str| {
            documented.exports(&path(module)).map(|all| all.into_iter().map(|e| e.name).collect::<Vec<_>>())
        };
        assert_eq!(names("low::deep"), Some(vec!["below".to_string()]));
        assert_eq!(names("nowhere"), None);
    }

    #[test]
    fn output_of_another_shape_is_refused() {
        let refused = |text: &str| Documented::read(text).err().unwrap_or_default();
        assert!(refused("{\"format_version\":99,\"root\":0}").contains("format 99"));
        assert!(refused("not json").starts_with("rustdoc's JSON cannot be read"));
    }
}
