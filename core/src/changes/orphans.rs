//! A base attestation with no counterpart at the head. Deleting a claim with its proof is
//! legitimate and silent; a claim that is still there while its proof is gone fails.

use std::collections::HashSet;

use super::change::{Change, ChangeType, Severity};
use super::side::{PageView, Side};
use super::text::{quote, similar, squash, substantive};
use super::units::{Located, Unit, line_in};
use crate::api::attestation::{Attestation, Body, Mode};
use crate::api::errors::Result;
use crate::text::{is_js_space, trim};

/// What became of a vanished attestation: an orphaned claim, an untagged block, or nothing.
pub fn lost_proof(a: &Attestation, base: &Side, head: &Side, pages: &[String]) -> Result<Option<Change>> {
    let counterparts = Counterparts { head, pages };
    if let Some((view, claim)) = base.claim(a)?
        && let Some(unit) = view.units.get(claim.unit)
        && substantive(&unit.text)
        && let Some(orphan) = counterparts.orphan(a, &view, &unit.text, claim)?
    {
        return Ok(Some(orphan));
    }
    counterparts.untagged(a)
}

/// Where at the head a vanished attestation's claim or block may still stand: the pages that
/// took over from its page.
struct Counterparts<'h, 's> {
    head: &'h Side<'s>,
    pages: &'h [String],
}

impl Counterparts<'_, '_> {
    fn orphan(&self, a: &Attestation, base: &PageView, text: &str, claim: Located) -> Result<Option<Change>> {
        // The same sentence may stand unproven elsewhere on the base page; only a new one counts.
        let before = base
            .units
            .iter()
            .enumerate()
            .filter(|(k, u)| u.text == text && !base.defended.contains(k))
            .count();
        let known: HashSet<&str> = base.units.iter().map(|u| u.text.as_str()).collect();
        for page in self.pages {
            let view = self.head.view(page)?;
            let open: Vec<&Unit> = view
                .units
                .iter()
                .enumerate()
                .filter(|(k, _)| !view.defended.contains(k))
                .map(|(_, u)| u)
                .collect();
            let same: Vec<&Unit> = open.iter().copied().filter(|u| u.text == text).collect();
            let found =
                if same.len() > before { same.get(before).copied() } else { reworded(text, &open, &known) };
            if let Some(found) = found {
                return Ok(Some(orphaned(a, page, line_in(found, claim.offset), text)));
            }
        }
        Ok(None)
    }

    // A check or compile block whose tag was removed is still on the page, as documentation.
    fn untagged(&self, a: &Attestation) -> Result<Option<Change>> {
        let Body::Example(example) = &a.body else { return Ok(None) };
        let code = squash(&example.code);
        for page in self.pages {
            let lines: HashSet<usize> = self.head.on(page).iter().map(|h| h.line).collect();
            let view = self.head.view(page)?;
            let block = view.code.iter().find(|c| {
                !lines.contains(&c.start) && squash(&c.value) == code && lang_of(&c.info) == example.lang
            });
            if let Some(block) = block {
                let mode = if example.mode == Mode::Check { "check" } else { "compile" };
                let message = format!("the `{mode}` block lost its tag: it is documentation now");
                return Ok(Some(Change::warning(ChangeType::Downgrade, page, block.start, message)));
            }
        }
        Ok(None)
    }
}

// A lightly reworded claim still counts, but only when exactly one new, unproven sentence
// is that close: when it is unclear whether the claim survived, the guard stays silent.
fn reworded<'u>(text: &str, open: &[&'u Unit], known: &HashSet<&str>) -> Option<&'u Unit> {
    let mut near =
        open.iter().copied().filter(|u| !known.contains(u.text.as_str()) && similar(text, &u.text));
    let first = near.next()?;
    near.next().is_none().then_some(first)
}

fn orphaned(a: &Attestation, page: &str, line: usize, claim: &str) -> Change {
    let quoted = quote(claim);
    let proof = format!("the {} that proved it ({}:{} at the base)", a.kind().as_str(), a.page, a.line);
    Change {
        kind: ChangeType::OrphanedClaim,
        page: page.to_string(),
        line,
        severity: Severity::Fail,
        message: format!("{quoted} is still on the page, but {proof} is gone"),
    }
}

/// The language of an info string: its first word once each `[…]` is blanked out.
fn lang_of(info: &str) -> String {
    let mut cleaned = String::with_capacity(info.len());
    let mut rest = info;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open + 1..].find(']') else { break };
        cleaned.push_str(&rest[..open]);
        cleaned.push(' ');
        rest = &rest[open + 1 + close + 1..];
    }
    cleaned.push_str(rest);
    trim(&cleaned).split(is_js_space).next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::lang_of;

    #[test]
    fn the_language_of_an_info_string_ignores_titles() {
        assert_eq!(lang_of("ts"), "ts");
        assert_eq!(lang_of("  ts twoslash"), "ts");
        assert_eq!(lang_of("[a.ts] ts check"), "ts");
        assert_eq!(lang_of("ts[title x]check"), "ts");
        assert_eq!(lang_of("[unclosed ts"), "[unclosed");
        assert_eq!(lang_of(""), "");
    }
}
