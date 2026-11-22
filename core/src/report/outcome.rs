//! The verdict on one attestation, or on one row of a cases table.

use serde_json::{Map, Value, json};

use crate::api::attestation::Kind;
use crate::api::errors::{Error, Result};

/// What a result says of its attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Error,
    Skipped,
}

impl Status {
    pub const ALL: [Status; 4] = [Status::Pass, Status::Fail, Status::Error, Status::Skipped];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::Error => "error",
            Status::Skipped => "skipped",
        }
    }

    fn parse(word: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|s| s.as_str() == word)
    }

    /// Whether a result of this status fails the check.
    pub fn is_failure(self) -> bool {
        matches!(self, Status::Fail | Status::Error)
    }
}

/// The verdict on one attestation (or one row of a cases table), as the protocol's `Result`
/// schema has it.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub page: String,
    pub line: usize,
    pub kind: Kind,
    pub status: Status,
    /// The surface symbols the attestation exercises; the core fills them in.
    pub symbols: Vec<String>,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl Outcome {
    /// A result located at an attestation (or at one of its rows).
    pub fn new(page: &str, line: usize, kind: Kind, status: Status, message: &str) -> Outcome {
        Outcome {
            page: page.to_string(),
            line,
            kind,
            status,
            symbols: Vec::new(),
            message: message.to_string(),
            expected: None,
            actual: None,
        }
    }

    pub fn with_comparison(mut self, expected: &str, actual: &str) -> Outcome {
        self.expected = Some(expected.to_string());
        self.actual = Some(actual.to_string());
        self
    }

    /// A result as an adapter answered it. One that does not fit the schema is the adapter's
    /// bug; the core has nothing of its own to put in its place.
    pub fn from_json(value: Value) -> Result<Outcome> {
        let Value::Object(fields) = value else {
            return Err(Error::internal("run answered a result that is not an object"));
        };
        let text = |key: &str| fields.get(key).and_then(Value::as_str);
        let missing =
            || Error::internal("run answered a result without a location, a kind, a status or a message");
        let page = text("page").filter(|p| !p.is_empty()).ok_or_else(missing)?;
        let line = fields.get("line").and_then(Value::as_u64).filter(|&l| l > 0).ok_or_else(missing)?;
        let kind = text("kind").and_then(Kind::parse).ok_or_else(missing)?;
        let status = text("status").and_then(Status::parse).ok_or_else(missing)?;
        let message = text("message").ok_or_else(missing)?;
        let mut outcome = Outcome::new(page, line as usize, kind, status, message);
        outcome.expected = text("expected").map(str::to_string);
        outcome.actual = text("actual").map(str::to_string);
        Ok(outcome)
    }

    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("page".into(), json!(self.page));
        out.insert("line".into(), json!(self.line));
        out.insert("kind".into(), json!(self.kind.as_str()));
        out.insert("status".into(), json!(self.status.as_str()));
        out.insert("symbols".into(), json!(self.symbols));
        out.insert("message".into(), json!(self.message));
        if let Some(expected) = &self.expected {
            out.insert("expected".into(), json!(expected));
        }
        if let Some(actual) = &self.actual {
            out.insert("actual".into(), json!(actual));
        }
        Value::Object(out)
    }

    pub fn is_failure(&self) -> bool {
        self.status.is_failure()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_from_the_adapter_is_read_into_its_fields() {
        let read = Outcome::from_json(json!({
            "page": "docs/a.md", "line": 3, "kind": "fact", "status": "fail",
            "message": "N is 4", "expected": "3", "actual": "4"
        }))
        .unwrap();
        assert_eq!(
            read,
            Outcome::new("docs/a.md", 3, Kind::Fact, Status::Fail, "N is 4").with_comparison("3", "4")
        );
        assert_eq!(
            read.to_json().to_string(),
            r#"{"page":"docs/a.md","line":3,"kind":"fact","status":"fail","symbols":[],"message":"N is 4","expected":"3","actual":"4"}"#
        );
    }

    #[test]
    fn a_result_that_does_not_fit_the_schema_is_the_adapters_bug() {
        let complete =
            json!({ "page": "docs/a.md", "line": 3, "kind": "fact", "status": "pass", "message": "ok" });
        for (key, wrong) in
            [("status", json!("maybe")), ("kind", json!("note")), ("line", json!(0)), ("page", json!(""))]
        {
            let mut result = complete.clone();
            result[key] = wrong;
            assert!(matches!(Outcome::from_json(result), Err(Error::Internal { .. })), "{key}");
        }
        assert!(matches!(Outcome::from_json(json!([])), Err(Error::Internal { .. })));
    }
}
