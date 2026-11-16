//! What running something proved, before it is placed on a page.

use crate::protocol::{Kind, Status, Verdict};

/// What running an example or a row of a table proved.
pub(super) struct Outcomes {
    pub(super) status: Status,
    pub(super) message: String,
    pub(super) expected: Option<String>,
    pub(super) actual: Option<String>,
}

impl Outcomes {
    pub(super) fn of(status: Status, message: impl Into<String>) -> Self {
        Outcomes { status, message: message.into(), expected: None, actual: None }
    }

    pub(super) fn pass(message: impl Into<String>) -> Self {
        Outcomes::of(Status::Pass, message)
    }

    pub(super) fn fail(message: impl Into<String>) -> Self {
        Outcomes::of(Status::Fail, message)
    }

    /// The outcome as the protocol's Result, at a line of a page.
    pub(super) fn verdict(self, page: &str, line: usize, kind: Kind) -> Verdict {
        Verdict {
            page: page.to_string(),
            line,
            kind,
            status: self.status,
            symbols: Vec::new(),
            expected: self.expected,
            actual: self.actual,
            message: self.message,
        }
    }
}
