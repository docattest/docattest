//! An impact as `docattest impact` prints it.

use super::commands::command_call;
use super::{Found, Impact};
use crate::api::attestation::Kind;
use crate::counted::counted;
use crate::manifests::manifest_call;
use crate::text::compare_utf16;

/// By symbol, then by changed manifest, then by changed command, each place with page, line, how it is defended, and
/// its sentence; then one line saying what to re-read and re-check.
pub fn format_impact(impact: &Impact) -> String {
    if impact.is_empty() {
        return format!("docattest impact: no exported symbol is affected by changes since {}", impact.base);
    }
    let mut lines =
        vec![format!("docattest impact: changes since {} affect {}", impact.base, affected(impact))];
    let manifests = impact.manifests.iter().map(|file| manifest_call(file));
    let commands = impact.commands.iter().map(|name| command_call(name));
    for subject in impact.symbols.iter().cloned().chain(manifests).chain(commands) {
        let references: Vec<_> = impact.references.iter().filter(|r| r.symbol == subject).collect();
        lines.push(subject);
        if references.is_empty() {
            lines.push("  no page names it".to_string());
        }
        for r in references {
            let how = match r.found {
                Found::Proven(kind) => format!("proven {}", kind.as_str()),
                Found::Cited => format!("cited {}", Kind::Test.as_str()),
                Found::Mention => "mention (unproven)".to_string(),
            };
            lines.push(format!("  {}:{} {how}  {}", r.page, r.line, r.claim));
        }
    }
    lines.push(next_step(impact));
    lines.join("\n")
}

/// How many exported symbols, manifests and commands the change affects, naming only those it
/// does.
fn affected(impact: &Impact) -> String {
    let count = |items: &[String], noun: &str| (!items.is_empty()).then(|| counted(items.len(), noun));
    let parts = [
        count(&impact.symbols, "exported symbol"),
        count(&impact.manifests, "manifest"),
        count(&impact.commands, "command"),
    ];
    let parts: Vec<String> = parts.into_iter().flatten().collect();
    match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => parts.join(""),
    }
}

fn next_step(impact: &Impact) -> String {
    let pages = unique(impact.references.iter().map(|r| r.page.as_str()));
    if pages.is_empty() {
        return "next: no living page names these symbols; nothing to re-read".to_string();
    }
    let checked = unique(impact.references.iter().filter(|r| r.found.is_checked()).map(|r| r.page.as_str()));
    let reread = format!(
        "next: re-read {} and correct any sentence above that the change made untrue",
        pages.join(", ")
    );
    if checked.is_empty() {
        reread
    } else {
        format!("{reread}; then run docattest check {}", checked.join(" "))
    }
}

fn unique<'a>(items: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = items.collect();
    out.sort_by(|a, b| compare_utf16(a, b));
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::super::ImpactReference;
    use super::*;

    #[test]
    fn groups_by_symbol_and_ends_with_what_to_re_read_and_re_check() {
        let reference = |page: &str, line, found, claim: &str| ImpactReference {
            symbol: "retry".into(),
            page: page.into(),
            line,
            found,
            claim: claim.into(),
        };
        let text = format_impact(&Impact {
            base: "HEAD".into(),
            symbols: vec!["parse".into(), "retry".into()],
            manifests: vec![],
            commands: vec![],
            references: vec![
                reference("docs/a.md", 7, Found::Proven(Kind::Example), "Works."),
                reference("docs/b.md", 2, Found::Mention, "Use it."),
                reference("docs/c.md", 4, Found::of(Kind::Test), "Retries back off."),
            ],
        });
        assert_eq!(
            text.split('\n').collect::<Vec<_>>(),
            vec![
                "docattest impact: changes since HEAD affect 2 exported symbols",
                "parse",
                "  no page names it",
                "retry",
                "  docs/a.md:7 proven example  Works.",
                "  docs/b.md:2 mention (unproven)  Use it.",
                "  docs/c.md:4 cited test  Retries back off.",
                "next: re-read docs/a.md, docs/b.md, docs/c.md and correct any sentence above that the change made untrue; then run docattest check docs/a.md docs/c.md",
            ]
        );
        assert_eq!(
            format_impact(&Impact {
                base: "main".into(),
                symbols: vec![],
                manifests: vec![],
                commands: vec![],
                references: vec![]
            }),
            "docattest impact: no exported symbol is affected by changes since main"
        );
    }

    #[test]
    fn a_changed_manifest_follows_the_symbols_with_the_facts_that_read_it() {
        let fact = ImpactReference {
            symbol: r#"manifest("package.json")"#.into(),
            page: "README.md".into(),
            line: 3,
            found: Found::Proven(Kind::Fact),
            claim: "Node 22 or later.".into(),
        };
        let impact = |symbols: Vec<String>| Impact {
            base: "HEAD".into(),
            symbols,
            manifests: vec!["package.json".into()],
            commands: vec![],
            references: vec![fact.clone()],
        };
        assert_eq!(
            format_impact(&impact(vec![])).split('\n').collect::<Vec<_>>(),
            vec![
                "docattest impact: changes since HEAD affect 1 manifest",
                r#"manifest("package.json")"#,
                "  README.md:3 proven fact  Node 22 or later.",
                "next: re-read README.md and correct any sentence above that the change made untrue; then run docattest check README.md",
            ]
        );
        let both = format_impact(&impact(vec!["parse".into(), "retry".into()]));
        assert_eq!(
            both.lines().next(),
            Some("docattest impact: changes since HEAD affect 2 exported symbols and 1 manifest")
        );
    }

    #[test]
    fn a_changed_command_comes_last_with_the_blocks_that_run_it() {
        let block = ImpactReference {
            symbol: "$ mytool".into(),
            page: "README.md".into(),
            line: 9,
            found: Found::Proven(Kind::Example),
            claim: "Help is printed.".into(),
        };
        let impact = Impact {
            base: "HEAD".into(),
            symbols: vec!["parse".into()],
            manifests: vec!["package.json".into()],
            commands: vec!["mytool".into()],
            references: vec![block],
        };
        let text = format_impact(&impact);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[..2],
            [
                "docattest impact: changes since HEAD affect 1 exported symbol, 1 manifest and 1 command",
                "parse"
            ]
        );
        assert_eq!(
            lines[lines.len() - 3..lines.len() - 1],
            ["$ mytool", "  README.md:9 proven example  Help is printed."]
        );
    }
}
