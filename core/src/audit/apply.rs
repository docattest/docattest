//! Applying `audit` suggestions to Markdown and MDX pages (`docattest audit --apply`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::{Status, Suggestion, Target};
use crate::api::errors::{Error, Result};
use crate::counted::counted;
use crate::markdown::text::sentence_ends;

/// Controls which audit suggestions are inserted and whether files are written.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyOptions {
    pub include_drift: bool,
    pub dry_run: bool,
}

/// One directive inserted (or planned in `--dry-run`) into a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedEdit {
    pub page: String,
    pub line: usize,
    pub target: String,
    pub status: String,
    pub directive: String,
}

impl AppliedEdit {
    pub fn to_json(&self) -> Value {
        json!({
            "page": self.page,
            "line": self.line,
            "target": self.target,
            "status": self.status,
            "directive": self.directive,
        })
    }
}

/// The outcome of applying audit suggestions across pages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub applied: Vec<AppliedEdit>,
    pub skipped_drift: usize,
    pub dry_run: bool,
}

impl ApplyReport {
    pub fn to_json(&self, suggestions: &[Suggestion]) -> Value {
        json!({
            "suggestions": suggestions.iter().map(Suggestion::to_json).collect::<Vec<_>>(),
            "applied": self.applied.iter().map(AppliedEdit::to_json).collect::<Vec<_>>(),
            "skippedDrift": self.skipped_drift,
            "dryRun": self.dry_run,
        })
    }
}

/// Insert directives for selected suggestions into pages under `root`.
pub fn apply_suggestions(
    root: &Path,
    suggestions: &[Suggestion],
    options: ApplyOptions,
) -> Result<ApplyReport> {
    let mut by_page: BTreeMap<&str, Vec<&Suggestion>> = BTreeMap::new();
    let mut skipped_drift = 0usize;
    for s in suggestions {
        if s.status == Status::Drift && !options.include_drift {
            skipped_drift += 1;
            continue;
        }
        by_page.entry(s.page.as_str()).or_default().push(s);
    }
    let mut applied = Vec::new();
    for (page, items) in by_page {
        let path = root.join(page);
        let original =
            fs::read_to_string(&path).map_err(|e| Error::user(format!("cannot read {page}: {e}")))?;
        let (updated, edits) = apply_to_text(&original, &items);
        if !options.dry_run && !edits.is_empty() && updated != original {
            fs::write(&path, updated).map_err(|e| Error::user(format!("cannot write {page}: {e}")))?;
        }
        applied.extend(edits);
    }
    Ok(ApplyReport { applied, skipped_drift, dry_run: options.dry_run })
}

/// Format an [`ApplyReport`] as human-readable text.
pub fn format_apply(report: &ApplyReport) -> String {
    let mut out = Vec::new();
    let mut page = "";
    for edit in &report.applied {
        if edit.page != page {
            page = &edit.page;
            out.push(page.to_string());
        }
        out.push(format!("  + L{} {}: {}", edit.line, edit.target, edit.directive));
    }
    let pages = unique_pages(&report.applied);
    let verb = if report.dry_run { "would apply" } else { "applied" };
    let mut summary = if report.applied.is_empty() {
        format!("audit: nothing to {}", if report.dry_run { "apply (dry run)" } else { "apply" })
    } else {
        format!("audit: {verb} {} to {}", counted(report.applied.len(), "directive"), counted(pages, "page"),)
    };
    if report.skipped_drift > 0 {
        summary
            .push_str(&format!(" ({} drift skipped; pass --include-drift to apply)", report.skipped_drift));
    }
    out.push(summary);
    out.join("\n")
}

fn unique_pages(edits: &[AppliedEdit]) -> usize {
    let mut seen = std::collections::HashSet::new();
    edits.iter().filter(|e| seen.insert(e.page.as_str())).count()
}

/// Apply a page's suggestions in reverse line order so earlier line numbers stay valid.
pub(crate) fn apply_to_text(text: &str, items: &[&Suggestion]) -> (String, Vec<AppliedEdit>) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_newline = text.ends_with('\n');
    let mut lines: Vec<String> = text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
    if trailing_newline && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let mut ordered: Vec<&Suggestion> = items.to_vec();
    ordered.sort_by_key(|a| std::cmp::Reverse(a.line));
    let mut applied = Vec::new();
    for s in ordered {
        if apply_one(&mut lines, s) {
            applied.push(AppliedEdit {
                page: s.page.clone(),
                line: s.line,
                target: s.target.as_str().to_string(),
                status: s.status.as_str().to_string(),
                directive: s.directive.clone(),
            });
        }
    }
    applied.reverse();
    let mut out = lines.join(eol);
    if trailing_newline {
        out.push_str(eol);
    }
    (out, applied)
}

fn apply_one(lines: &mut Vec<String>, s: &Suggestion) -> bool {
    if s.line == 0 || s.line > lines.len() {
        return false;
    }
    let idx = s.line - 1;
    match &s.target {
        Target::List | Target::Column | Target::Block => {
            let indent: String = lines[idx].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            lines.insert(idx, format!("{indent}{}", s.directive));
            true
        }
        Target::Inline => {
            lines[idx] = insert_after_sentence(&lines[idx], &s.directive);
            true
        }
        Target::Scalar { after } => {
            let Some(updated) = insert_after_literal(&lines[idx], after, &s.directive) else {
                return false;
            };
            lines[idx] = updated;
            true
        }
    }
}

fn insert_after_sentence(line: &str, directive: &str) -> String {
    if let Some(end) = sentence_ends(line).next() {
        let mut out = String::with_capacity(line.len() + directive.len() + 1);
        out.push_str(&line[..end]);
        out.push(' ');
        out.push_str(directive);
        out.push_str(&line[end..]);
        return out;
    }
    format!("{line} {directive}")
}

fn insert_after_literal(line: &str, after: &str, directive: &str) -> Option<String> {
    let bare_number = !after.starts_with('`');
    let mut from = 0;
    while let Some(rel) = line.get(from..)?.find(after) {
        let start = from + rel;
        let end = start + after.len();
        if !inside_comment(line, start) && (!bare_number || is_standalone_token(line, start, end)) {
            let mut out = String::with_capacity(line.len() + directive.len() + 1);
            out.push_str(&line[..end]);
            out.push(' ');
            out.push_str(directive);
            out.push_str(&line[end..]);
            return Some(out);
        }
        from = start + 1;
    }
    None
}

fn is_standalone_token(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    let is_num_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
    !before.is_some_and(is_num_char) && !after.is_some_and(is_num_char)
}

fn inside_comment(line: &str, pos: usize) -> bool {
    let before = &line[..pos];
    let in_html = before.rfind("<!--").is_some_and(|open| before[open..].find("-->").is_none());
    let in_jsx = before.rfind("{/*").is_some_and(|open| before[open..].find("*/}").is_none());
    in_html || in_jsx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggestion(line: usize, target: Target, directive: &str) -> Suggestion {
        Suggestion {
            page: "docs/p.md".into(),
            line,
            target,
            expression: "X".into(),
            status: Status::Match,
            missing: Vec::new(),
            unexpected: Vec::new(),
            surface: "default".into(),
            directive: directive.into(),
        }
    }

    #[test]
    fn inserts_list_inline_and_scalar_directives_without_shifting_lines() {
        let page = "Regions:\n\n- `eu`\n- `us`\n\nPort 8080 is used. Tags `a`, `b` are valid. More.\n";
        let items = [
            suggestion(3, Target::List, "<!-- fact: REGIONS -->"),
            suggestion(6, Target::Scalar { after: "8080".into() }, "<!-- fact: PORT -->"),
            suggestion(6, Target::Inline, "<!-- fact: TAGS -->"),
        ];
        let refs: Vec<&Suggestion> = items.iter().collect();
        let (out, applied) = apply_to_text(page, &refs);
        assert_eq!(applied.len(), 3);
        assert!(out.contains("<!-- fact: REGIONS -->\n- `eu`"));
        assert!(out.contains("Port 8080 <!-- fact: PORT --> is used."));
    }

    #[test]
    fn skips_drift_unless_include_drift_is_set() {
        let dir = std::env::temp_dir().join(format!("docattest-apply-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let page = dir.join("README.md");
        fs::write(&page, "- `a`\n- `b`\n").unwrap();
        let mut drift = suggestion(1, Target::List, "<!-- fact: K -->");
        drift.page = "README.md".into();
        drift.status = Status::Drift;

        let skipped = apply_suggestions(&dir, std::slice::from_ref(&drift), ApplyOptions::default()).unwrap();
        assert_eq!(skipped.applied.len(), 0);
        assert_eq!(skipped.skipped_drift, 1);
        assert_eq!(fs::read_to_string(&page).unwrap(), "- `a`\n- `b`\n");

        let all = apply_suggestions(
            &dir,
            std::slice::from_ref(&drift),
            ApplyOptions { include_drift: true, dry_run: false },
        )
        .unwrap();
        assert_eq!(all.applied.len(), 1);
        assert_eq!(fs::read_to_string(&page).unwrap(), "<!-- fact: K -->\n- `a`\n- `b`\n");
        let _ = fs::remove_dir_all(&dir);
    }
}
