//! A base attestation with no counterpart at the head. Deleting a claim with its proof is
//! legitimate and silent; a claim that is still there while its proof is gone fails, whatever
//! other proofs stay in its paragraph.

use std::collections::HashSet;

use super::change::{Change, ChangeType, Severity};
use super::side::{PageView, Side};
use super::text::{quote, similar, squash, substantive};
use super::units::{claim_piece, line_in};
use crate::api::attestation::{Attestation, Body, Mode};
use crate::api::errors::Result;
use crate::text::{is_js_space, trim};

/// What became of a vanished attestation: an orphaned claim, an untagged block, or nothing.
/// `fresh` holds the ids of the head attestations no base attestation became.
pub fn lost_proof(
    a: &Attestation,
    base: &Side,
    head: &Side,
    pages: &[String],
    fresh: &HashSet<&str>,
) -> Result<Option<Change>> {
    let counterparts = Counterparts { head, pages, fresh };
    if let Some(orphan) = counterparts.orphan(a, base)? {
        return Ok(Some(orphan));
    }
    counterparts.untagged(a)
}

/// Where at the head a vanished attestation's claim or block may still stand: the pages that
/// took over from its page.
struct Counterparts<'h, 's> {
    head: &'h Side<'s>,
    pages: &'h [String],
    fresh: &'h HashSet<&'h str>,
}

/// A piece of a page's prose (a sentence, or a whole unit) and the unit it is in.
type Piece<'v> = (usize, &'v str);

impl Counterparts<'_, '_> {
    fn orphan(&self, a: &Attestation, base: &Side) -> Result<Option<Change>> {
        let Some((view, claim)) = base.claim(a)? else { return Ok(None) };
        let piece = claim_piece(a, &view.units[claim.unit]);
        if !substantive(piece) {
            return Ok(None);
        }
        // The same claim may stand elsewhere on the base page, unproven or proven another way;
        // only a new one counts.
        let before = open_pieces(&view, |b| b.id != a.id && proves_alike(a, b))
            .into_iter()
            .filter(|&(unit, text)| text == piece && unit != claim.unit)
            .count();
        let known = &view.piece_texts;
        for page in self.pages {
            let view = self.head.view(page)?;
            // A new proof of the claim, or one like the lost proof, keeps it proven.
            let open = open_pieces(&view, |h| self.fresh.contains(h.id.as_str()) || proves_alike(a, h));
            if let Some((unit, _)) = surviving(piece, &open, before, known) {
                return Ok(Some(orphaned(a, page, line_in(&view.units[unit], claim.offset), piece)));
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

/// The pieces of a page's prose that no `standing` attestation claims.
fn open_pieces<'v>(view: &'v PageView<'_>, standing: impl Fn(&Attestation) -> bool) -> Vec<Piece<'v>> {
    let claimed: HashSet<Piece> = (view.claims.iter())
        .filter(|(h, _)| standing(h))
        .map(|(_, (unit, text))| (*unit, text.as_str()))
        .collect();
    let all = view.pieces.iter().map(|(unit, text)| (*unit, text.as_str()));
    all.filter(|piece| !claimed.contains(piece)).collect()
}

/// The unproven piece of a head page that is the lost proof's claim: a copy beyond the
/// `before` copies the base page already had, else a light rewording of it.
fn surviving<'v>(
    piece: &str,
    open: &[Piece<'v>],
    before: usize,
    known: &HashSet<String>,
) -> Option<Piece<'v>> {
    let same: Vec<Piece> = open.iter().copied().filter(|&(_, text)| text == piece).collect();
    if same.len() > before { same.get(before).copied() } else { reworded(piece, open, known) }
}

/// Another proof of the same kind bound to the same claim: two facts on one literal.
fn proves_alike(a: &Attestation, other: &Attestation) -> bool {
    other.kind() == a.kind() && other.claim == a.claim
}

// A lightly reworded claim still counts, but only when exactly one new, unproven piece of
// prose is that close: when it is unclear whether the claim survived, the guard stays silent.
fn reworded<'v>(text: &str, open: &[Piece<'v>], known: &HashSet<String>) -> Option<Piece<'v>> {
    let mut near = open.iter().copied().filter(|&(_, p)| !known.contains(p) && similar(text, p));
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
