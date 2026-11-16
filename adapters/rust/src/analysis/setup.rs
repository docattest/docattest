//! The setup module as syntax: the names of its `pub` items, which fact expressions may
//! use, and of the helpers among them that `observables` calls.

use super::Analysis;
use crate::language::doc::named;
use syn::{Item, ReturnType, Visibility};

impl Analysis {
    /// The setup file as a syntax tree; `None` when none is configured or it cannot be
    /// read.
    pub(crate) fn setup_file(&self) -> Option<syn::File> {
        let text = std::fs::read_to_string(self.abs(self.setup.as_ref()?)).ok()?;
        syn::parse_file(&text).ok()
    }

    /// The names of the setup module's `pub` items.
    pub(crate) fn setup_names(&self) -> Vec<String> {
        let public = |item: &Item| match item {
            Item::Fn(f) => matches!(f.vis, Visibility::Public(_)),
            Item::Const(c) => matches!(c.vis, Visibility::Public(_)),
            Item::Static(s) => matches!(s.vis, Visibility::Public(_)),
            _ => false,
        };
        let items = self.setup_file().map(|file| file.items).unwrap_or_default();
        items
            .iter()
            .filter(|item| public(item))
            .filter_map(|item| named(item).map(|(name, _)| name))
            .collect()
    }

    /// The setup module's `pub` functions that take no arguments and return a value, are
    /// neither generic nor `async`: what `observables` calls, since they exist for pages to
    /// read. Nothing else is ever called to see what it returns.
    pub(crate) fn setup_helpers(&self) -> Vec<String> {
        let items = self.setup_file().map(|file| file.items).unwrap_or_default();
        items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(f)
                    if matches!(f.vis, Visibility::Public(_))
                        && f.sig.inputs.is_empty()
                        && f.sig.generics.params.is_empty()
                        && f.sig.asyncness.is_none()
                        && !matches!(f.sig.output, ReturnType::Default) =>
                {
                    Some(f.sig.ident.to_string())
                }
                _ => None,
            })
            .collect()
    }
}
