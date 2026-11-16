//! `inventory`: what the surfaces offer a page.

use super::InventoryEntry;
use super::run::{can_observe, request_values};
use crate::api::errors::{Error, Result};
use crate::config::DEFAULT_SURFACE;
use crate::counted::counted;
use crate::project::check::Network;
use crate::project::check::open;
use crate::project::state::State;

/// Each value a fact could read, with the expression that reads it. `search` keeps the
/// entries whose expression or value contains it. A surface that cannot be loaded is an
/// error, as it is for `audit`.
pub fn run_inventory(state: &State, network: Network, search: Option<&str>) -> Result<Vec<InventoryEntry>> {
    if !can_observe(state) {
        return Err(Error::user("this adapter cannot list what a surface offers"));
    }
    let opened = open(state, network, &[]);
    let offered = request_values(state);
    opened.wait()?;
    let entries = offered.wait()?.into_iter().map(|o| InventoryEntry {
        surface: o.surface,
        expression: o.expression,
        value: o.value.text,
        items: o.value.items,
    });
    Ok(match search {
        None => entries.collect(),
        Some(search) => {
            let needle = search.to_lowercase();
            entries.filter(|entry| holds(entry, &needle)).collect()
        }
    })
}

/// Whether the expression, or any item (or the value), holds `needle` once lower-cased.
fn holds(entry: &InventoryEntry, needle: &str) -> bool {
    let has = |text: &str| text.to_lowercase().contains(needle);
    has(&entry.expression)
        || match &entry.items {
            Some(items) => items.iter().any(|item| has(item)),
            None => has(&entry.value),
        }
}

/// One line per entry: the expression, then the items or the value it reads.
pub fn format_inventory(entries: &[InventoryEntry]) -> String {
    if entries.is_empty() {
        return "inventory: nothing matches".into();
    }
    let mut lines: Vec<String> = entries
        .iter()
        .map(|entry| {
            let label =
                if entry.surface == DEFAULT_SURFACE { String::new() } else { format!("@{} ", entry.surface) };
            let shown = match &entry.items {
                Some(items) => format!("[{}]", items.join(", ")),
                None => entry.value.clone(),
            };
            format!("{label}{}  →  {shown}", entry.expression)
        })
        .collect();
    lines.push(format!("inventory: {}", counted(entries.len(), "value")));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(surface: &str, expression: &str, value: &str, items: Option<&[&str]>) -> InventoryEntry {
        InventoryEntry {
            surface: surface.into(),
            expression: expression.into(),
            value: value.into(),
            items: items.map(|i| i.iter().map(|s| (*s).to_string()).collect()),
        }
    }

    #[test]
    fn a_search_reads_expressions_and_items_or_values_whatever_the_case() {
        let list = entry("default", "KEYS", "Alpha,BETA", Some(&["Alpha", "BETA"]));
        assert!(holds(&list, "keys"));
        assert!(holds(&list, "beta"));
        // A list is searched by its items, not by its text.
        assert!(!holds(&list, "alpha,beta"));
        let text = entry("default", "NAME", "Größe", None);
        assert!(holds(&text, &"GRÖSSE".to_lowercase()) || !"größe".contains("grösse"));
        assert!(holds(&text, &"GRÖ".to_lowercase()));
        assert!(!holds(&text, "nothing"));
        assert!(holds(&text, ""));
    }

    #[test]
    fn prints_one_line_per_entry_and_a_count() {
        let entries = [
            entry("default", "KEYS", "a,b", Some(&["a", "b"])),
            entry("server", "PORT", "8080", None),
            entry("default", "NONE", "", Some(&[])),
        ];
        assert_eq!(
            format_inventory(&entries),
            "KEYS  →  [a, b]\n@server PORT  →  8080\nNONE  →  []\ninventory: 3 values"
        );
        assert_eq!(format_inventory(&[]), "inventory: nothing matches");
        assert_eq!(format_inventory(&entries[..1]), "KEYS  →  [a, b]\ninventory: 1 value");
    }
}
