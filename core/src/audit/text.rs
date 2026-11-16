//! An audit as `docattest audit` prints it.

use super::{Status, Suggestion, Target};
use crate::audit::distinct;

fn place(target: &Target) -> String {
    match target {
        Target::List => "above the list".into(),
        Target::Column => "above the table".into(),
        Target::Block => "above the block".into(),
        Target::Inline => "at the end of the sentence".into(),
        Target::Scalar { after } => format!("after {after}"),
    }
}

/// Suggestions by page, then one summary line.
pub fn format_audit(suggestions: &[Suggestion]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut page = "";
    for s in suggestions {
        if s.page != page {
            page = &s.page;
            out.push(page.to_string());
        }
        let (line, target) = (s.line, s.target.as_str());
        out.push(format!("  ? L{line} {target} {} → {} {}", verdict(s), s.directive, place(&s.target)));
    }
    out.push(summary(suggestions));
    out.join("\n")
}

fn verdict(s: &Suggestion) -> String {
    if s.status == Status::Match {
        return format!("matches {}", s.expression);
    }
    let mut parts = Vec::new();
    if !s.missing.is_empty() {
        parts.push(format!("missing: {}", s.missing.join(", ")));
    }
    if !s.unexpected.is_empty() {
        parts.push(format!("unexpected: {}", s.unexpected.join(", ")));
    }
    format!("drifts from {} ({})", s.expression, parts.join("; "))
}

fn summary(all: &[Suggestion]) -> String {
    if all.is_empty() {
        return "audit: nothing to suggest".into();
    }
    let drift = all.iter().filter(|s| s.status == Status::Drift).count();
    let pages = distinct(all.iter().map(|s| s.page.as_str())).len();
    let noun = if all.len() == 1 { "suggestion" } else { "suggestions" };
    let page_noun = if pages == 1 { "page" } else { "pages" };
    format!("audit: {} {noun} on {pages} {page_noun}, {} match, {drift} drift", all.len(), all.len() - drift)
}

#[cfg(test)]
mod tests {
    use super::super::suggest::tests::{suggest, value};
    use super::*;

    #[test]
    fn prints_pages_one_line_per_suggestion_and_a_summary() {
        let page = "A token may carry:\n\n- `read`\n- `write`\n- `billing`\n";
        let suggestions = suggest(page, &[value("SCOPES", &["read", "write"])]);
        assert_eq!(
            format_audit(&suggestions),
            [
                "docs/p.md",
                "  ? L3 list drifts from SCOPES (unexpected: billing) → <!-- fact: SCOPES --> above the list",
                "audit: 1 suggestion on 1 page, 0 match, 1 drift",
            ]
            .join("\n")
        );
        assert_eq!(format_audit(&[]), "audit: nothing to suggest");
    }
}
