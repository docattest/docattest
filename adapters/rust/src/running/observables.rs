//! What a surface offers (`observables`): exports and setup helpers observed by a generated program.

use super::Runner;
use super::facts::{DEFAULT_SURFACE, Scope};
use super::generated::Dependencies;
use crate::analysis::surfaces::Surface;
use crate::protocol::{Error, Observable, Observation};
use serde_json::{Value, json};

/// Hexadecimal digits after `0x` that make an address: fewer is a number someone wrote.
const MIN_ADDRESS_DIGITS: usize = 6;

impl Runner {
    /// Answers `observables`: the values a surface offers, which are those of its
    /// exports that can be shown, each with the fact expression that reads it. A surface
    /// that cannot be read is the project's mistake.
    pub(crate) fn observables(&self, name: &str) -> Result<Value, Error> {
        let user = |why: String| Error::User(format!("docattest: {why}"));
        if let Some(missing) = self.analysis.not_found(name) {
            return Err(user(missing));
        }
        let Some(surface) = self.analysis.surface(name) else { return Ok(json!([])) };
        let roots = self.roots(surface);
        if roots.is_empty() {
            return Ok(json!([]));
        }
        // An export that is not a value (a type, a function) does not compile as one,
        // and is taken out of the program like a fact that does not compile.
        let scope = Scope { surface: Some(surface), dependencies: Dependencies::Project };
        let program =
            self.build_program(roots.len(), &|live| self.facts_main(scope, &roots, live, "observables"));
        let Some(binary) = program.binary else {
            return program
                .failure
                .map_or(Ok(json!([])), |why| Err(user(format!("surface \"{name}\" failed to load: {why}"))));
        };
        let answers = self
            .serve(&binary, None, 1)
            .map_err(|why| user(format!("surface \"{name}\" could not be read: {why}")))?;
        let answer = &answers[0];
        if let Some(why) =
            answer.panic.clone().or(answer.lost.clone()).or(answer.timeout.then(|| self.timed_out()))
        {
            return Err(user(format!("surface \"{name}\" could not be read: {why}")));
        }
        let found: Vec<Observable> = answer
            .value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| observable(name, item))
            .filter(|o| !shows_an_address(&o.value))
            .collect();
        Ok(json!(found))
    }

    /// What `observables` reads: a surface's exports and, on the default surface, a call of
    /// each setup helper whose name the surface does not export (a fact could not use it).
    fn roots(&self, surface: &Surface) -> Vec<String> {
        let mut roots = self.analysis.exports_of(&surface.name).unwrap_or_default();
        if surface.name == DEFAULT_SURFACE {
            let helpers = self.analysis.setup_helpers().into_iter();
            roots.extend(helpers.filter(|h| !self.analysis.exported(surface, h)).map(|h| format!("{h}()")));
        }
        roots
    }
}

/// Whether a value that is not text shows an address (`0x16b0f2f48`), as a pointer or a
/// function pointer does: it is shown differently in each run, so it is not offered.
fn shows_an_address(value: &Observation) -> bool {
    if value.kind.as_deref() == Some("text") {
        return false;
    }
    let text = value.text.as_bytes();
    text.windows(2).enumerate().any(|(k, pair)| {
        let starts = k == 0 || !(text[k - 1].is_ascii_alphanumeric() || text[k - 1] == b'_');
        let hex = text[k + 2..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
        starts && pair == b"0x" && hex >= MIN_ADDRESS_DIGITS
    })
}

/// One value a program read, as the protocol's Observable.
fn observable(surface: &str, item: &Value) -> Option<Observable> {
    Some(Observable {
        surface: surface.to_string(),
        expression: item["expression"].as_str()?.to_string(),
        path: vec![item["expression"].as_str()?.to_string()],
        value: serde_json::from_value(item["value"].clone()).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str, kind: Option<&str>) -> Observation {
        Observation {
            text: text.into(),
            items: None,
            description: String::new(),
            kind: kind.map(str::to_string),
        }
    }

    #[test]
    fn a_value_that_shows_an_address_is_not_offered() {
        assert!(shows_an_address(&value("[(\"a\", 0x1004f43c0)]", None)));
        assert!(shows_an_address(&value("0x16b0f2f48", None)));
        for shown in ["255", "0xFF", "x0x1004f43c0", "[\"a\"]"] {
            assert!(!shows_an_address(&value(shown, None)), "{shown}");
        }
        assert!(!shows_an_address(&value("0x1004f43c0", Some("text"))));
    }
}
