//! One thing a change did to the attestations that existed at the base, and the changes
//! section of the reports.

use serde_json::{Value, json};

use crate::counted::counted;

/// The kinds of change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    OrphanedClaim,
    FrozenPage,
    Downgrade,
    Weakened,
}

impl ChangeType {
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeType::OrphanedClaim => "orphaned-claim",
            ChangeType::FrozenPage => "frozen-page",
            ChangeType::Downgrade => "downgrade",
            ChangeType::Weakened => "weakened",
        }
    }
}

/// `fail` sets exit code 1; a `warning` never changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Fail,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Fail => "fail",
            Severity::Warning => "warning",
        }
    }
}

/// One change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeType,
    /// The page as it is now.
    pub page: String,
    /// The line now: the claim's, the block's or the fact's; 1 for a whole page.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

impl Change {
    pub fn warning(kind: ChangeType, page: &str, line: usize, message: String) -> Change {
        Change { kind, page: page.to_string(), line, severity: Severity::Warning, message }
    }

    /// The JSON form, keys in the order the recordings hold.
    pub fn to_json(&self) -> Value {
        json!({
            "type": self.kind.as_str(),
            "page": self.page,
            "line": self.line,
            "severity": self.severity.as_str(),
            "message": self.message,
        })
    }
}

/// The "changes" section of a text report; nothing when the check had no base.
pub fn change_lines(changes: Option<&[Change]>) -> Vec<String> {
    let Some(changes) = changes else { return Vec::new() };
    if changes.is_empty() {
        return vec!["changes since the base: none".into()];
    }
    let fails = changes.iter().filter(|c| c.severity == Severity::Fail).count();
    let warnings = counted(changes.len() - fails, "warning");
    let mut lines = vec![format!("changes since the base: {fails} fail, {warnings}")];
    lines.extend(changes.iter().map(|c| {
        let mark = if c.severity == Severity::Fail { '✗' } else { '⚠' };
        format!("  {mark} {}:{} {}: {}", c.page, c.line, c.kind.as_str(), c.message)
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orphan() -> Change {
        Change {
            kind: ChangeType::OrphanedClaim,
            page: "docs/a.md".into(),
            line: 3,
            severity: Severity::Fail,
            message: "\"N is `3`.\" is still on the page, but the fact that proved it is gone".into(),
        }
    }

    #[test]
    fn the_section_is_absent_without_a_base_and_says_none_when_nothing_changed() {
        assert!(change_lines(None).is_empty());
        assert_eq!(change_lines(Some(&[])), vec!["changes since the base: none"]);
    }

    #[test]
    fn each_change_is_listed_with_its_page_line_and_type() {
        let frozen =
            Change { kind: ChangeType::FrozenPage, line: 1, severity: Severity::Warning, ..orphan() };
        let lines = change_lines(Some(&[orphan(), frozen]));
        assert_eq!(lines[0], "changes since the base: 1 fail, 1 warning");
        assert!(lines[1].starts_with("  ✗ docs/a.md:3 orphaned-claim: \"N is `3`.\" is still on the page"));
        assert!(lines[2].starts_with("  ⚠ docs/a.md:1 frozen-page:"));
    }

    #[test]
    fn json_keeps_the_recorded_key_order() {
        assert_eq!(
            orphan().to_json().to_string(),
            r#"{"type":"orphaned-claim","page":"docs/a.md","line":3,"severity":"fail","message":"\"N is `3`.\" is still on the page, but the fact that proved it is gone"}"#
        );
    }
}
