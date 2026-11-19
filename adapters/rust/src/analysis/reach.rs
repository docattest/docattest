//! What code reaches through the items of the file it is written in: a cases runner, a
//! setup function a fact calls, a docs helper. Each is credited with what it uses of the
//! surfaces through the helpers it calls, as if their code were its own.

use super::affected::owner;
use crate::language::text;
use quote::ToTokens;
use syn::Item;

/// The items some names reach among a file's items: those of the names, then those of
/// the names their code writes that `followed` lets through, at any depth. An `impl`
/// block is part of its type. Read from tokens, so a name stands for any item of it.
pub(super) fn reached<'a>(
    items: &'a [Item],
    roots: &[String],
    followed: &dyn Fn(&str) -> bool,
) -> Vec<&'a Item> {
    let mut names: Vec<String> = roots.to_vec();
    let mut found = Vec::new();
    let mut next = 0;
    while let Some(name) = names.get(next).cloned() {
        next += 1;
        for item in items.iter().filter(|item| owner(item).as_deref() == Some(name.as_str())) {
            found.push(item);
            let tokens = item.to_token_stream().to_string();
            for written in text::identifiers(&tokens) {
                if followed(written) && !names.iter().any(|known| known == written) {
                    names.push(written.to_string());
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_reaches_the_items_it_names_at_any_depth() {
        let file = syn::parse_file(
            "fn runner() { helper(); }\nfn helper() -> u8 { LIMIT + Kind::new().0 }\nconst LIMIT: u8 = 1;\nstruct Kind(u8);\nimpl Kind { fn new() -> Kind { Kind(DEEP) } }\nconst DEEP: u8 = 2;\nfn unrelated() {}\npub fn stop() { unrelated(); }",
        )
        .unwrap();
        let names = |roots: &[&str], followed: &dyn Fn(&str) -> bool| {
            let roots: Vec<String> = roots.iter().map(|root| root.to_string()).collect();
            let found = reached(&file.items, &roots, followed);
            found.into_iter().filter_map(owner).collect::<Vec<_>>()
        };
        assert_eq!(names(&["runner"], &|_| true), ["runner", "helper", "LIMIT", "Kind", "Kind", "DEEP"]);
        assert_eq!(names(&["runner", "stop"], &|name| name != "helper"), ["runner", "stop", "unrelated"]);
    }
}
