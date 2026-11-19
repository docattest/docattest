//! Docs helpers: the items of a module under `#[cfg(docattest)]`.
//!
//! docattest builds the project's crates with that cfg, and nothing else does, so such an
//! item exists only for the pages. As code of its module it reads what the module keeps
//! private, and a fact calls a `pub` one by its name, as it calls an export: the glob
//! import of the surface's module brings it in. It is no export, so `uncovered` does not
//! ask a page for it.

use super::Analysis;
use super::modules::{Crate, is_public, visibility};
use super::surfaces::Surface;
use crate::language::doc::named;
use crate::toolchain::cargo::DOCS_CFG;
use syn::Item;

/// A docs helper, as far as a session keeps it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct DocsHelper {
    pub(super) name: String,
    /// Whether it is `pub`, with no restriction: a fact's program, a crate of its own,
    /// reaches nothing else.
    public: bool,
}

/// Whether an item is a docs helper: it has a name and is declared under
/// `#[cfg(docattest)]`.
pub(super) fn is_docs_helper(item: &Item) -> bool {
    let under_cfg = |attr: &syn::Attribute| {
        attr.path().is_ident("cfg") && attr.parse_args::<syn::Ident>().is_ok_and(|cfg| cfg == DOCS_CFG)
    };
    named(item).is_some_and(|(_, attrs)| attrs.iter().any(under_cfg))
}

/// The docs helpers among a module's items.
pub(super) fn docs_helpers(items: &[Item]) -> Vec<DocsHelper> {
    let helpers = items.iter().filter(|item| is_docs_helper(item));
    let found = helpers.filter_map(|item| {
        let (name, _) = named(item)?;
        Some(DocsHelper { name, public: visibility(item).is_some_and(is_public) })
    });
    found.collect()
}

impl Analysis {
    /// The docs helper of a surface's module of a name, when it declares one.
    pub(super) fn docs_helper(&self, surface: &Surface, name: &str) -> Option<DocsHelper> {
        let located = surface.located()?;
        let summary = self.summary(located.owner.name(), located.owner.root());
        summary.docs_helpers(&located.module).iter().find(|helper| helper.name == name).cloned()
    }

    /// Whether a fact may call a docs helper of its surface: it is `pub`, in a library,
    /// which the fact's program imports.
    pub(super) fn callable_helper(&self, surface: &Surface, name: &str) -> bool {
        surface.importable() && self.docs_helper(surface, name).is_some_and(|helper| helper.public)
    }

    /// Why a fact's program could not find a docs helper of its surface, when the name is
    /// one.
    pub(super) fn unread_helper(&self, surface: &Surface, name: &str) -> Option<String> {
        self.docs_helper(surface, name)?;
        let at = &surface.name;
        if !self.callable_helper(surface, name) {
            return Some(format!(
                "{name} is a docs helper of surface \"{at}\" that a fact cannot call: a fact's program reaches a docs helper only when it is `pub`, in a module of a library"
            ));
        }
        Some(format!(
            "{name} is a docs helper of surface \"{at}\", and the build did not set `--cfg {DOCS_CFG}`: a cargo config's `target.<triple>.rustflags` takes the place of the `build.rustflags` docattest adds it to"
        ))
    }

    /// The items of a surface's module, read again: a session keeps no syntax tree.
    pub(super) fn module_items(&self, surface: &Surface) -> Option<Vec<Item>> {
        let located = surface.located()?;
        let read = |path: &std::path::Path| std::fs::read_to_string(path).ok();
        let krate = Crate::load(located.owner.name(), located.owner.root(), &read);
        Some(krate.module(&located.module)?.items.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_docs_helper_is_an_item_under_the_docs_cfg() {
        let file = syn::parse_file(
            "#[cfg(docattest)]\npub fn names() -> Vec<&'static str> { vec![] }\n#[cfg(docattest)]\nfn hidden() {}\n#[cfg(test)]\npub fn tested() {}\n#[cfg(any(docattest, test))]\npub fn either() {}\npub fn plain() {}",
        )
        .unwrap();
        let helper = |name: &str, public| DocsHelper { name: name.to_string(), public };
        assert_eq!(docs_helpers(&file.items), [helper("names", true), helper("hidden", false)]);
    }
}
