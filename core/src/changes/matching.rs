//! Pairing base attestations with head attestations by what they are and what they defend, never by
//! line: prose added above a proof moves it, and that is not a change.

use std::cell::OnceCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::side::Side;
use super::signature::{proof_of, target_of};
use super::text::{dice, similar, substantive, words};
use crate::api::attestation::Attestation;
use crate::api::errors::Result;

/// The result of matching. Attestations are indexes into each side's plan.
pub struct Matching {
    /// A base attestation and the head attestation it became.
    pub pairs: Vec<(usize, usize)>,
    /// Base attestations with no counterpart at the head.
    pub unmatched: Vec<usize>,
    renamed: HashMap<String, Vec<String>>,
}

impl Matching {
    /// The head pages a base page lives on now: itself if still living, or where it went.
    pub fn counterparts(&self, head: &Side, page: &str) -> Vec<String> {
        counterparts(&self.renamed, head, page)
    }
}

fn counterparts(renamed: &HashMap<String, Vec<String>>, head: &Side, page: &str) -> Vec<String> {
    let mut pages = Vec::new();
    if head.living(page) {
        pages.push(page.to_string());
    }
    pages.extend(renamed.get(page).into_iter().flatten().cloned());
    pages
}

struct Matchable<'a> {
    a: &'a Attestation,
    proof: String,
    proof_words: OnceCell<Vec<String>>,
    target: String,
    /// The prose the attestation defends, when it is prose.
    claim: Option<String>,
}

impl Matchable<'_> {
    fn proof_words(&self) -> &[String] {
        self.proof_words.get_or_init(|| words(&self.proof))
    }
}

/// A base and a head attestation that may be the same one, and how well they fit.
struct Candidate {
    b: usize,
    h: usize,
    rank: [f64; 4],
}

/// Two passes. First, the same proof anywhere (a moved section, a renamed page, a downgrade);
/// that also tells which page became which. Then, the same claim on the same page (or where
/// it went), with an edited proof: the pairs a weakening shows up in.
pub fn match_attestations(base: &Side, head: &Side) -> Result<Matching> {
    let b = infos(base)?;
    let h = infos(head)?;
    let mut pairs = greedy(same_proof(&b, &h));
    let renamed = renames(&pairs, &b, &h, head);
    let used_b: HashSet<usize> = pairs.iter().map(|p| p.0).collect();
    let used_h: HashSet<usize> = pairs.iter().map(|p| p.1).collect();
    let rest_b: Vec<usize> = (0..b.len()).filter(|k| !used_b.contains(k)).collect();
    let rest_h: Vec<usize> = (0..h.len()).filter(|k| !used_h.contains(k)).collect();
    pairs.extend(greedy(same_claim(&b, &h, &rest_b, &rest_h, |page| counterparts(&renamed, head, page))));
    let matched: HashSet<usize> = pairs.iter().map(|p| p.0).collect();
    let unmatched = (0..b.len()).filter(|k| !matched.contains(k)).collect();
    Ok(Matching { pairs, unmatched, renamed })
}

fn infos<'a>(side: &Side<'a>) -> Result<Vec<Matchable<'a>>> {
    side.project
        .attestations
        .iter()
        .map(|a| {
            let claim = side
                .claim(a)?
                .and_then(|(view, located)| view.units.get(located.unit).map(|u| u.text.clone()));
            Ok(Matchable { a, proof: proof_of(a), proof_words: OnceCell::new(), target: target_of(a), claim })
        })
        .collect()
}

fn flag(yes: bool) -> f64 {
    f64::from(u8::from(yes))
}

fn apart(b: &Attestation, h: &Attestation) -> f64 {
    -(b.line.abs_diff(h.line) as f64)
}

fn same_proof(base: &[Matchable], head: &[Matchable]) -> Vec<Candidate> {
    let mut by_proof: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    for (k, h) in head.iter().enumerate() {
        by_proof.entry((h.a.kind().as_str(), h.proof.as_str())).or_default().push(k);
    }
    let mut candidates = Vec::new();
    for (k, b) in base.iter().enumerate() {
        for &j in by_proof.get(&(b.a.kind().as_str(), b.proof.as_str())).into_iter().flatten() {
            let h = &head[j];
            candidates.push(Candidate {
                b: k,
                h: j,
                rank: [
                    flag(b.claim.is_some() && b.claim == h.claim),
                    flag(b.target == h.target),
                    flag(b.a.page == h.a.page),
                    apart(b.a, h.a),
                ],
            });
        }
    }
    candidates
}

fn same_claim(
    base: &[Matchable],
    head: &[Matchable],
    rest_b: &[usize],
    rest_h: &[usize],
    counterparts: impl Fn(&str) -> Vec<String>,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for &k in rest_b {
        let b = &base[k];
        let Some(claim) = b.claim.as_deref().filter(|c| substantive(c)) else { continue };
        let pages = counterparts(&b.a.page);
        for &j in rest_h {
            let h = &head[j];
            let Some(theirs) = h.claim.as_deref() else { continue };
            if h.a.kind() != b.a.kind() || !pages.contains(&h.a.page) {
                continue;
            }
            let same = claim == theirs;
            if !same && !similar(claim, theirs) {
                continue;
            }
            let close = dice(b.proof_words(), h.proof_words());
            candidates.push(Candidate { b: k, h: j, rank: [flag(same), close, apart(b.a, h.a), 0.0] });
        }
    }
    candidates
}

// Best candidates first (the sort is stable, so equal ranks keep the order candidates were
// found in); each attestation pairs at most once.
fn greedy(mut candidates: Vec<Candidate>) -> Vec<(usize, usize)> {
    candidates.sort_by(|x, y| compare_ranks(&y.rank, &x.rank));
    let (mut used_b, mut used_h) = (HashSet::new(), HashSet::new());
    let mut pairs = Vec::new();
    for c in candidates {
        if used_b.contains(&c.b) || used_h.contains(&c.h) {
            continue;
        }
        used_b.insert(c.b);
        used_h.insert(c.h);
        pairs.push((c.b, c.h));
    }
    pairs
}

fn compare_ranks(x: &[f64; 4], y: &[f64; 4]) -> Ordering {
    for (p, q) in x.iter().zip(y) {
        match p.partial_cmp(q) {
            Some(Ordering::Equal) | None => {}
            Some(order) => return order,
        }
    }
    Ordering::Equal
}

// A base page that is no longer living, whose proofs turned up on another page, went there.
fn renames(
    pairs: &[(usize, usize)],
    base: &[Matchable],
    head: &[Matchable],
    side: &Side,
) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for &(b, h) in pairs {
        let (from, to) = (&base[b].a.page, &head[h].a.page);
        if from == to || side.living(from) {
            continue;
        }
        let pages = out.entry(from.clone()).or_default();
        if !pages.contains(to) {
            pages.push(to.clone());
        }
    }
    out
}
