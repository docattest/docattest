//! Showing a value as Rust displays it, without knowing its type.
//!
//! A fact expression may have any type, and a generated program has no reflection to
//! ask what it is. [`observe!`] lets method resolution choose instead: each way of
//! showing a value is a trait implemented for a different depth of reference to
//! [`Wrap`], and a call on a reference finds the implementations in a fixed order (for
//! `&&Wrap`: the one for `&Wrap`, then `&&Wrap`, then `Wrap`), skipping those whose
//! bounds the type does not meet.
//!
//! Text is `Display` when the type has it, else `Debug`. Items are the keys of a map or
//! of a list of pairs, else the items of anything a reference to which can be iterated,
//! each shown the same way; a value that is text has none.

use super::json::{Json, quote};
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Display};

/// A value as the core compares it (the protocol's Observation).
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub text: String,
    pub items: Option<Vec<String>>,
    pub is_text: bool,
    pub description: String,
}

impl Observation {
    /// The observation as JSON text.
    pub fn to_json(&self) -> String {
        let mut fields = vec![format!("\"text\":{}", quote(&self.text))];
        if let Some(items) = self.items.as_ref().filter(|_| !self.is_text) {
            fields.push(format!(
                "\"items\":[{}]",
                items.iter().map(|i| quote(i)).collect::<Vec<_>>().join(",")
            ));
        }
        fields.push(format!("\"description\":{}", quote(&self.description)));
        if self.is_text {
            fields.push("\"kind\":\"text\"".into());
        }
        format!("{{{}}}", fields.join(","))
    }
}

/// A reference to the value being shown.
pub struct Wrap<T>(pub T);

/// A value that is itself a reference (a `&[&str]` constant, a `&str`) is shown as what
/// it refers to, so that a borrowed list has its items as an owned one has.
pub trait Borrowed<'b, T: ?Sized> {
    fn peeled(&self) -> Wrap<&'b T>;
}
impl<'b, T: ?Sized> Borrowed<'b, T> for Wrap<&&'b T> {
    fn peeled(&self) -> Wrap<&'b T> {
        Wrap(*self.0)
    }
}
pub trait Owned<'a, T: ?Sized> {
    fn peeled(&self) -> Wrap<&'a T>;
}
impl<'a, T: ?Sized> Owned<'a, T> for &Wrap<&'a T> {
    fn peeled(&self) -> Wrap<&'a T> {
        Wrap(self.0)
    }
}

pub trait TextByDisplay {
    fn text(&self) -> String;
}
impl<T: Display + ?Sized> TextByDisplay for Wrap<&T> {
    fn text(&self) -> String {
        self.0.to_string()
    }
}
pub trait TextByDebug {
    fn text(&self) -> String;
}
impl<T: Debug + ?Sized> TextByDebug for &Wrap<&T> {
    fn text(&self) -> String {
        format!("{:?}", self.0)
    }
}

pub trait KeysOfMap {
    fn items(&self) -> Option<Vec<String>>;
}
impl<K: Display + Ord, V> KeysOfMap for &&&Wrap<&BTreeMap<K, V>> {
    fn items(&self) -> Option<Vec<String>> {
        Some(self.0.keys().map(K::to_string).collect())
    }
}
impl<K: Display + Ord, V, S> KeysOfMap for &&&Wrap<&HashMap<K, V, S>> {
    fn items(&self) -> Option<Vec<String>> {
        let mut keys: Vec<&K> = self.0.keys().collect();
        keys.sort();
        Some(keys.into_iter().map(K::to_string).collect())
    }
}
impl KeysOfMap for &&&Wrap<&Json> {
    fn items(&self) -> Option<Vec<String>> {
        match self.0 {
            Json::Array(items) => Some(items.iter().map(Json::to_string).collect()),
            Json::Object(_) => Some(self.0.keys().into_iter().map(str::to_string).collect()),
            _ => None,
        }
    }
}
pub trait KeysOfPairs {
    fn items(&self) -> Option<Vec<String>>;
}
impl<'a, T: ?Sized, K: Display + 'a, V: 'a> KeysOfPairs for &&&&Wrap<&'a T>
where
    &'a T: IntoIterator<Item = &'a (K, V)>,
{
    fn items(&self) -> Option<Vec<String>> {
        Some(self.0.into_iter().map(|(key, _)| key.to_string()).collect())
    }
}
pub trait ItemsByDisplay {
    fn items(&self) -> Option<Vec<String>>;
}
impl<'a, T: ?Sized> ItemsByDisplay for &&Wrap<&'a T>
where
    &'a T: IntoIterator,
    <&'a T as IntoIterator>::Item: Display,
{
    fn items(&self) -> Option<Vec<String>> {
        Some(self.0.into_iter().map(|item| item.to_string()).collect())
    }
}
pub trait ItemsByDebug {
    fn items(&self) -> Option<Vec<String>>;
}
impl<'a, T: ?Sized> ItemsByDebug for &Wrap<&'a T>
where
    &'a T: IntoIterator,
    <&'a T as IntoIterator>::Item: Debug,
{
    fn items(&self) -> Option<Vec<String>> {
        Some(self.0.into_iter().map(|item| format!("{item:?}")).collect())
    }
}
pub trait NoItems {
    fn items(&self) -> Option<Vec<String>>;
}
impl<T: ?Sized> NoItems for Wrap<&T> {
    fn items(&self) -> Option<Vec<String>> {
        None
    }
}

pub trait IsText {
    fn is_text(&self) -> bool;
}
impl<T: AsRef<str> + ?Sized> IsText for Wrap<&T> {
    fn is_text(&self) -> bool {
        true
    }
}
/// A string of a snapshot or a manifest is text, as a `String` is.
impl IsText for Wrap<&Json> {
    fn is_text(&self) -> bool {
        matches!(self.0, Json::String(_))
    }
}
pub trait NotText {
    fn is_text(&self) -> bool;
}
impl<T: ?Sized> NotText for &Wrap<&T> {
    fn is_text(&self) -> bool {
        false
    }
}

/// The name of a value's type, for a message.
pub fn type_name<T: ?Sized>(_: &T) -> String {
    std::any::type_name::<T>().to_string()
}

/// A value as an [`Observation`].
#[macro_export]
macro_rules! observe {
    ($value:expr) => {{
        #[allow(unused_imports)]
        use $crate::harness::observe::*;
        // A match keeps the expression's temporaries alive while the value is read.
        match $value {
            ref value => {
                let shown = (&Wrap(value)).peeled();
                Observation {
                    text: (&shown).text(),
                    items: (&&&&shown).items(),
                    is_text: (&shown).is_text(),
                    description: type_name(value),
                }
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::super::json;

    fn shown(o: super::Observation) -> (String, Option<Vec<String>>, bool) {
        (o.text, o.items, o.is_text)
    }

    fn list(items: &[&str]) -> Option<Vec<String>> {
        Some(items.iter().map(|i| i.to_string()).collect())
    }

    #[test]
    fn scalars_are_displayed_and_have_no_items() {
        assert_eq!(shown(observe!(3)), ("3".into(), None, false));
        assert_eq!(shown(observe!(10_000.0)), ("10000".into(), None, false));
        assert_eq!(shown(observe!(true)), ("true".into(), None, false));
        assert_eq!(shown(observe!("a b")), ("a b".into(), None, true));
        assert_eq!(shown(observe!(String::from("s"))), ("s".into(), None, true));
    }

    #[test]
    fn lists_show_each_item_as_it_displays() {
        assert_eq!(shown(observe!(["b", "a"])), ("[\"b\", \"a\"]".into(), list(&["b", "a"]), false));
        assert_eq!(shown(observe!(vec![1, 2])).1, list(&["1", "2"]));
        assert_eq!(shown(observe!(vec![Some(1), None])).1, list(&["Some(1)", "None"]));
        assert_eq!(shown(observe!(Vec::<u8>::new())), ("[]".into(), list(&[]), false));
        const BORROWED: &[&str] = &["x", "y"];
        assert_eq!(shown(observe!(BORROWED)).1, list(&["x", "y"]));
        assert_eq!(shown(observe!(&vec![1, 2])).1, list(&["1", "2"]));
    }

    #[test]
    fn maps_and_lists_of_pairs_show_their_keys() {
        const PAIRS: [(&str, u8); 2] = [("b", 1), ("a", 2)];
        assert_eq!(shown(observe!(PAIRS)).1, list(&["b", "a"]));
        assert_eq!(shown(observe!(&PAIRS[..])).1, list(&["b", "a"]));
        assert_eq!(shown(observe!(HashMap::from([("b", 1), ("a", 2)]))).1, list(&["a", "b"]));
        assert_eq!(shown(observe!(BTreeMap::from([(2, "x"), (1, "y")]))).1, list(&["1", "2"]));
    }

    #[test]
    fn a_temporary_in_the_expression_lives_long_enough() {
        assert_eq!(shown(observe!(BTreeMap::from([("a", 1)]).keys().collect::<Vec<_>>())).1, list(&["a"]));
    }

    #[test]
    fn json_shows_as_a_page_states_it() {
        let data = json::parse(r#"[{"code":"NL"},{"code":"DE"}]"#).unwrap();
        let codes: Vec<String> = data.iter().map(|region| region["code"].to_string()).collect();
        assert_eq!(shown(observe!(codes)).1, list(&["NL", "DE"]));
        assert_eq!(shown(observe!(json::parse(r#"["a",2]"#).unwrap())).1, list(&["a", "2"]));
        assert_eq!(shown(observe!(json::parse(r#"{"b":1,"a":2}"#).unwrap())).1, list(&["b", "a"]));
        let manifest = json::parse(r#"{"version":"1.2.0","n":3}"#).unwrap();
        assert_eq!(shown(observe!(manifest["version"])), ("1.2.0".into(), None, true));
        assert_eq!(shown(observe!(manifest["n"])), ("3".into(), None, false));
        let written = observe!(vec!["a\"b"]).to_json();
        assert_eq!(json::parse(&written).unwrap()["items"][0].as_str(), Some("a\"b"));
    }
}
