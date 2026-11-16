//! Everything the surfaces offer, indexed so a page set is only compared with the surface lists it
//! shares items with. Nested objects multiply the lists; the index keeps the cost of a page
//! proportional to what it could match, not to the size of the surface.

use std::collections::HashMap;

use super::matching::{MIN_BLOCK_LENGTH, collapse};
use super::{Observable, distinct};

/// Surface values `audit` compares pages with, in surface order.
pub struct Inventory<'a> {
    /// Lists: the default surface first, then each export as declared, shallow before deep.
    pub lists: Vec<&'a Observable>,
    /// Strings long enough for a fenced block to copy.
    pub strings: Vec<&'a Observable>,
    by_item: HashMap<&'a str, Vec<usize>>,
    /// Values that are not lists, by their text.
    by_text: HashMap<&'a str, Vec<&'a Observable>>,
}

impl<'a> Inventory<'a> {
    pub fn new(observables: &'a [Observable]) -> Inventory<'a> {
        let mut inventory = Inventory {
            lists: Vec::new(),
            strings: Vec::new(),
            by_item: HashMap::new(),
            by_text: HashMap::new(),
        };
        for o in observables {
            let Some(items) = &o.value.items else {
                inventory.by_text.entry(o.value.text.as_str()).or_default().push(o);
                let long = collapse(&o.value.text).chars().count() >= MIN_BLOCK_LENGTH;
                if o.value.is_text && long {
                    inventory.strings.push(o);
                }
                continue;
            };
            let at = inventory.lists.len();
            inventory.lists.push(o);
            for item in distinct(items.iter().map(String::as_str)) {
                inventory.by_item.entry(item).or_default().push(at);
            }
        }
        inventory
    }

    /// Lists sharing at least `min` distinct items with `items`, in surface order.
    pub fn sharing(&self, items: &[String], min: usize) -> Vec<&'a Observable> {
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for item in distinct(items.iter().map(String::as_str)) {
            for at in self.by_item.get(item).map_or(&[][..], Vec::as_slice) {
                *counts.entry(*at).or_default() += 1;
            }
        }
        let mut found: Vec<usize> = counts.into_iter().filter(|(_, n)| *n >= min).map(|(at, _)| at).collect();
        found.sort_unstable();
        found.into_iter().filter_map(|at| self.lists.get(at).copied()).collect()
    }

    /// The one value that is not a list and whose text is `text`; None when there is none,
    /// or when several are and nothing says which one a page copied.
    pub fn only_value(&self, text: &str) -> Option<&'a Observable> {
        match self.by_text.get(text).map(Vec::as_slice) {
            Some([only]) => Some(*only),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::matching::tests::{observe_list, observe_text, strings};
    use super::*;

    fn value(expression: &str, value: crate::compare::Observation) -> Observable {
        Observable { surface: "default".into(), expression: expression.into(), value }
    }

    #[test]
    fn indexes_lists_and_keeps_long_strings() {
        let all = [
            value("A", observe_list(&["x1", "x2", "x2"])),
            value("SHORT", observe_text("too short")),
            value("LONG", observe_text("long   enough\n to be   copied")),
            value("B", observe_list(&["x2", "x3", "x1"])),
            value("C", observe_list(&["x3", "x4"])),
        ];
        let inventory = Inventory::new(&all);
        let names =
            |found: Vec<&Observable>| -> Vec<String> { found.iter().map(|o| o.expression.clone()).collect() };
        assert_eq!(names(inventory.strings.clone()), ["LONG"]);
        assert_eq!(names(inventory.sharing(&strings(&["x2", "x1", "x1"]), 2)), ["A", "B"]);
        assert_eq!(names(inventory.sharing(&strings(&["x2", "x2"]), 2)), Vec::<String>::new());
        assert_eq!(names(inventory.sharing(&strings(&["x3"]), 1)), ["B", "C"]);
    }

    #[test]
    fn finds_the_only_value_with_a_text() {
        let all = [
            value("PORT", observe_text("8080")),
            value("A", observe_text("EUR")),
            value("B", observe_text("EUR")),
            value("LIST", observe_list(&["x1", "x2"])),
        ];
        let inventory = Inventory::new(&all);
        assert_eq!(inventory.only_value("8080").map(|o| o.expression.as_str()), Some("PORT"));
        assert_eq!(inventory.only_value("EUR"), None);
        assert_eq!(inventory.only_value("x1,x2"), None);
        assert_eq!(inventory.only_value("9090"), None);
    }

    #[test]
    fn a_string_needs_sixteen_collapsed_utf16_units() {
        let all = [
            value("A", observe_text("1234567  \n 90123456")),
            value("B", observe_text("123456   \n   9012345")),
        ];
        let inventory = Inventory::new(&all);
        assert_eq!(inventory.strings.len(), 1);
    }
}
