//! The text report of `mutate`: for each page, a line for each proof, ✓ killed, ✗ survived
//! (with each mutant it survived below it) and - not mutated (with why), then a summary. A
//! count of mutants says how many of them were stillborn, when any were.

use super::verdict::{Judged, Mutation, Verdict};
use crate::counted::counted;

/// The report, as `mutate` prints it.
pub fn format_mutation(mutation: &Mutation) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut page: Option<&str> = None;
    for proof in &mutation.proofs {
        if page != Some(proof.page.as_str()) {
            lines.push(proof.page.clone());
            page = Some(&proof.page);
        }
        lines.extend(proof_lines(proof));
    }
    let (killed, survived, not_mutated) = mutation.counts();
    lines.push(format!(
        "mutants: {}{}; proofs: {killed} killed, {survived} survived, {not_mutated} not mutated",
        mutation.mutants,
        stillborn_note(mutation.stillborn)
    ));
    lines.join("\n")
}

fn proof_lines(proof: &Judged) -> Vec<String> {
    let row = proof.row.as_ref().map(|name| format!(" \"{name}\"")).unwrap_or_default();
    let at = format!("L{} {}{row}", proof.line, proof.kind.as_str());
    match &proof.verdict {
        Verdict::Killed { by, tried, stillborn, ignored } => {
            let mut line = format!(
                "  ✓ {at}: killed by {by} ({}{})",
                counted(*tried, "mutant"),
                stillborn_note(*stillborn)
            );
            if !ignored.is_empty() {
                line.push_str(&format!("; no change of {} made it fail", cells(ignored)));
            }
            vec![line]
        }
        Verdict::Survived { survivors, stillborn } => {
            let survived = counted(survivors.len(), "mutant");
            let mut out = vec![format!("  ✗ {at}: survived {survived}{}", stillborn_note(*stillborn))];
            out.extend(survivors.iter().map(|s| format!("      {s}")));
            out
        }
        Verdict::NotMutated { why } => vec![format!("  - {at}: not mutated: {why}")],
    }
}

/// `, 2 stillborn` after a count of mutants, or nothing when none was.
fn stillborn_note(stillborn: usize) -> String {
    if stillborn == 0 { String::new() } else { format!(", {stillborn} stillborn") }
}

/// `cell a`, `cells a and b`, `cells a, b and c`.
fn cells(columns: &[String]) -> String {
    match columns {
        [one] => format!("cell {one}"),
        [rest @ .., last] => format!("cells {} and {last}", rest.join(", ")),
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::attestation::Kind;

    fn judged(line: usize, row: Option<&str>, verdict: Verdict) -> Judged {
        Judged {
            page: "docs/a.md".into(),
            line,
            kind: if row.is_some() { Kind::Cases } else { Kind::Fact },
            claim: String::new(),
            row: row.map(str::to_string),
            verdict,
        }
    }

    #[test]
    fn each_proof_has_a_line_and_a_survivor_lists_what_it_survived() {
        let mutation = Mutation {
            proofs: vec![
                judged(
                    1,
                    None,
                    Verdict::Killed {
                        by: "src/a.ts:1 PORT: 8080 → 8081".into(),
                        tried: 1,
                        stillborn: 1,
                        ignored: vec![],
                    },
                ),
                judged(
                    3,
                    None,
                    Verdict::Survived {
                        survivors: vec!["src/a.ts:2 NAME: 'shop' → 'shopx'".into()],
                        stillborn: 0,
                    },
                ),
                judged(5, None, Verdict::NotMutated { why: "size is declared with no literal".into() }),
                judged(
                    9,
                    Some("two"),
                    Verdict::Killed {
                        by: "cell b: 1 → 2".into(),
                        tried: 3,
                        stillborn: 0,
                        ignored: vec!["c".into(), "d".into()],
                    },
                ),
            ],
            mutants: 7,
            stillborn: 1,
        };
        assert_eq!(
            format_mutation(&mutation),
            [
                "docs/a.md",
                "  ✓ L1 fact: killed by src/a.ts:1 PORT: 8080 → 8081 (1 mutant, 1 stillborn)",
                "  ✗ L3 fact: survived 1 mutant",
                "      src/a.ts:2 NAME: 'shop' → 'shopx'",
                "  - L5 fact: not mutated: size is declared with no literal",
                "  ✓ L9 cases \"two\": killed by cell b: 1 → 2 (3 mutants); no change of cells c and d made it fail",
                "mutants: 7, 1 stillborn; proofs: 2 killed, 1 survived, 1 not mutated",
            ]
            .join("\n")
        );
    }
}
