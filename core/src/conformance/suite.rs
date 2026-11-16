//! What every check is given, and what a check is. [`super::run_suite`] makes one [`Suite`]
//! for a run and hands it to each check in turn; nothing a check needs is read from anywhere
//! else.

use std::cell::OnceCell;

use serde_json::Value;

use super::Adapter;
use super::expect::{Checked, ensure};
use super::kit::Kit;
use super::requests::Requests;
use super::talk::{Conversation, Line, talk};
use crate::schema::{METHODS, Schemas};

/// One check: its name, and a conversation it holds to what it states.
#[derive(Clone, Copy)]
pub struct Check {
    pub name: &'static str,
    pub run: fn(&Suite) -> Checked<Conversation>,
}

/// The kit, the adapter to talk to, the requests built from the kit, and the schemas.
pub struct Suite<'a> {
    pub kit: &'a Kit,
    pub requests: Requests<'a>,
    schemas: &'a Schemas,
    adapter: &'a Adapter<'a>,
    /// The greeting the first check hears. The checks after it compare with it, so they depend
    /// on running after the first.
    greeting: OnceCell<Value>,
}

impl<'a> Suite<'a> {
    pub fn new(kit: &'a Kit, adapter: &'a Adapter<'a>, schemas: &'a Schemas) -> Suite<'a> {
        Suite { kit, requests: Requests::new(kit), schemas, adapter, greeting: OnceCell::new() }
    }

    /// One conversation with the adapter: `lines` in, then the input ends.
    pub fn talk(&self, lines: impl IntoIterator<Item = Line>) -> Checked<Conversation> {
        talk(self.adapter, lines.into_iter().collect(), self.schemas)
    }

    /// A conversation of hello, open with `examples`, and `requests` with ids from 3.
    pub fn session(&self, requests: &[(&str, Value)], examples: Value) -> Checked<Conversation> {
        self.talk(self.requests.session(requests, examples).into_iter().map(Line::from))
    }

    /// `value` fits the definition `definition` of the methods' schemas.
    pub fn fits(&self, value: &Value, definition: &str) -> Checked {
        let problems = self.schemas.check(value, definition, METHODS);
        ensure(problems.is_empty(), format!("{definition}: {}", problems.join("; ")))
    }

    /// Keeps the greeting the first check heard.
    pub fn hear(&self, greeting: &Value) {
        // Only the first check hears one, so it is never already set.
        let _ = self.greeting.set(greeting.clone());
    }

    /// The greeting heard, which the first check must have heard.
    pub fn greeting(&self) -> Checked<&Value> {
        self.greeting.get().ok_or_else(|| "no greeting: the first hello check did not pass".to_string())
    }
}
