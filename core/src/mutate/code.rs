//! The mutants of code: what the adapter answers to `mutants` (PROTOCOL.md §3), read and
//! held to the protocol. Only the adapter knows how a literal of its language is written, so
//! it says what to change; the core applies each change and runs the proofs again.

use serde_json::{Value, json};

use crate::adapter::{Adapter, Pending};
use crate::api::errors::{Error, Result};
use crate::project::check::SymbolRef;

/// The capability, and the method, of an adapter that changes code.
pub const CAPABILITY: &str = "mutants";

/// One change to one file of the project, as the adapter answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub path: String,
    /// Byte offsets in the file's UTF-8 text, `end` excluded.
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    /// What changes, as the language writes it: `8080 → 8081`.
    pub description: String,
}

/// Why a symbol has no edits, as the adapter said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEdits {
    /// A function, method, class or type: what a proof reads of it is computed when it runs.
    Function,
    /// A value declared with no literal.
    NoLiteral,
    /// Nothing the project's own files declare.
    NotFound,
}

impl NoEdits {
    pub const ALL: [NoEdits; 3] = [NoEdits::Function, NoEdits::NoLiteral, NoEdits::NotFound];

    /// Why the symbols `names` (at least one) were not changed, as the report says it.
    pub fn phrase(self, names: &[String]) -> String {
        let one = names.len() == 1;
        let listed = match names {
            [rest @ .., last] if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => names.join(""),
        };
        let is = if one { "is" } else { "are" };
        match (self, one) {
            (NoEdits::Function, true) => {
                format!("{listed} is a function, class or type: what it gives is computed when it runs")
            }
            (NoEdits::Function, false) => {
                format!("{listed} are functions, classes or types: what they give is computed when they run")
            }
            (NoEdits::NoLiteral, _) => format!("{listed} {is} declared with no literal"),
            (NoEdits::NotFound, _) => format!("{listed} {is} declared in none of the project's own files"),
        }
    }
}

/// The adapter's answer for one symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolEdits {
    Edits(Vec<Edit>),
    None(NoEdits),
}

/// Ask for at most `limit` edits of each of `symbols`.
pub fn ask(adapter: &Adapter, symbols: &[SymbolRef], limit: usize) -> Pending {
    let symbols: Vec<Value> = symbols.iter().map(symbol_json).collect();
    adapter.send(CAPABILITY, json!({ "symbols": symbols, "limit": limit }))
}

/// A symbol as `symbols` answered it.
fn symbol_json(symbol: &SymbolRef) -> Value {
    match &symbol.surface {
        Some(surface) => json!({ "name": symbol.name, "surface": surface }),
        None => json!({ "name": symbol.name }),
    }
}

/// The answer to [`ask`], one entry for each of the `asked` symbols. An answer that breaks
/// the protocol is the adapter's bug.
pub fn read_answer(answer: Value, asked: usize) -> Result<Vec<SymbolEdits>> {
    let Value::Array(entries) = answer else {
        return Err(Error::internal("mutants did not answer a list"));
    };
    if entries.len() != asked {
        return Err(Error::internal(format!(
            "mutants answered {} entries for {asked} symbols",
            entries.len()
        )));
    }
    entries.iter().map(symbol_edits).collect()
}

fn symbol_edits(entry: &Value) -> Result<SymbolEdits> {
    let edits = entry.get("edits").and_then(Value::as_array);
    let edits = edits.ok_or_else(|| Error::internal("mutants answered an entry without edits"))?;
    let edits: Vec<Edit> = edits.iter().map(edit).collect::<Result<_>>()?;
    let why = entry.get("why").and_then(Value::as_str);
    match (edits.is_empty(), why) {
        (false, None) => Ok(SymbolEdits::Edits(edits)),
        (true, Some("function")) => Ok(SymbolEdits::None(NoEdits::Function)),
        (true, Some("noLiteral")) => Ok(SymbolEdits::None(NoEdits::NoLiteral)),
        (true, Some("notFound")) => Ok(SymbolEdits::None(NoEdits::NotFound)),
        _ => Err(Error::internal(
            "mutants answered an entry whose why is not there exactly when it has no edits",
        )),
    }
}

fn edit(value: &Value) -> Result<Edit> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    let offset = |key: &str| value.get(key).and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok());
    let read = (text("path"), offset("start"), offset("end"), text("replacement"), text("description"));
    let (Some(path), Some(start), Some(end), Some(replacement), Some(description)) = read else {
        return Err(Error::internal(
            "mutants answered an edit without a path, offsets, a replacement or a description",
        ));
    };
    Ok(Edit { path, start, end, replacement, description })
}

impl Edit {
    /// `text` with this edit made; an edit outside it, or inside a character, is the
    /// adapter's bug.
    pub fn applied_to(&self, text: &str) -> Result<String> {
        let inside = self.start <= self.end
            && self.end <= text.len()
            && text.is_char_boundary(self.start)
            && text.is_char_boundary(self.end);
        if !inside {
            return Err(Error::internal(format!(
                "mutants answered an edit of {} at {}..{}, which is not within its text",
                self.path, self.start, self.end
            )));
        }
        Ok(format!("{}{}{}", &text[..self.start], self.replacement, &text[self.end..]))
    }

    /// The line of `text` the edit starts on, counted from 1.
    pub fn line_in(&self, text: &str) -> usize {
        text.as_bytes()[..self.start.min(text.len())].iter().filter(|b| **b == b'\n').count() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit_json(start: u64, end: u64) -> Value {
        json!({ "path": "a.ts", "start": start, "end": end, "replacement": "4", "description": "3 → 4" })
    }

    #[test]
    fn an_answer_gives_each_symbol_its_edits_or_why_it_has_none() {
        let answer = json!([{ "edits": [edit_json(10, 11)] }, { "edits": [], "why": "function" }]);
        let read = read_answer(answer, 2).unwrap();
        let SymbolEdits::Edits(edits) = &read[0] else { panic!("{read:?}") };
        assert_eq!(edits[0].applied_to("const N = 3;").unwrap(), "const N = 4;");
        assert_eq!(edits[0].line_in("\n\nconst N = 3;"), 3);
        assert_eq!(read[1], SymbolEdits::None(NoEdits::Function));
    }

    #[test]
    fn symbols_with_no_edits_for_one_reason_are_named_together() {
        let names = |list: &[&str]| -> Vec<String> { list.iter().map(|n| n.to_string()).collect() };
        assert_eq!(
            NoEdits::Function.phrase(&names(&["Money", "total", "quote"])),
            "Money, total and quote are functions, classes or types: what they give is computed when they run"
        );
        assert_eq!(NoEdits::NoLiteral.phrase(&names(&["CARRIERS"])), "CARRIERS is declared with no literal");
        assert_eq!(
            NoEdits::NotFound.phrase(&names(&["a", "b"])),
            "a and b are declared in none of the project's own files"
        );
    }

    #[test]
    fn an_answer_that_breaks_the_protocol_is_the_adapters_bug() {
        let wrong = [
            (json!({}), 1),
            (json!([]), 1),
            (json!([{ "edits": [] }]), 1),
            (json!([{ "edits": [edit_json(0, 1)], "why": "notFound" }]), 1),
            (json!([{ "edits": [], "why": "because" }]), 1),
            (json!([{ "edits": [{ "path": "a.ts" }] }]), 1),
        ];
        for (answer, asked) in wrong {
            assert!(matches!(read_answer(answer.clone(), asked), Err(Error::Internal { .. })), "{answer}");
        }
        let outside =
            Edit { path: "a".into(), start: 1, end: 2, replacement: String::new(), description: "x".into() };
        assert!(outside.applied_to("é").is_err());
        assert!(outside.applied_to("").is_err());
    }

    #[test]
    fn a_symbol_is_asked_for_as_symbols_answered_it() {
        let with = SymbolRef { name: "N".into(), surface: Some("default".into()) };
        let without = SymbolRef { name: "PORT".into(), surface: None };
        assert_eq!(symbol_json(&with), json!({ "name": "N", "surface": "default" }));
        assert_eq!(symbol_json(&without), json!({ "name": "PORT" }));
    }
}
