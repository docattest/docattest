//! Finding and binding every attestation on a Markdown page.

use std::collections::HashSet;

use super::bind::{bind_cases, bind_fact};
use super::citation::Citation;
use super::core_examples::plan_core_example;
use super::directives::{Directive, parse_directive};
use super::examples::{ExampleOptions, plan_example};
use crate::api::attestation::{Attestation, Body, Cases, Fact, Test};
use crate::config::DEFAULT_SURFACE;
use crate::markdown::model::{BlockKind, Comment, Keyword, Page, Unclosed};
use crate::markdown::parse::{Positionless, parse_page};
use crate::markdown::text::sentence_ends;
use crate::text::trim;

/// What one page holds.
#[derive(Debug, Clone, PartialEq)]
pub struct PagePlan {
    /// Frontmatter `docattest: false`.
    pub frozen: bool,
    /// In line order.
    pub attestations: Vec<Attestation>,
}

/// Find and bind every attestation on a Markdown page.
pub fn plan_page(page: &str, text: &str, options: &ExampleOptions) -> Result<PagePlan, Positionless> {
    let parsed = parse_page(text)?;
    if parsed.frozen {
        return Ok(PagePlan { frozen: true, attestations: Vec::new() });
    }
    Ok(PagePlan { frozen: false, attestations: plan_parsed(page, &parsed, options) })
}

/// The attestations of a parsed page that is not frozen, in line order.
pub fn plan_parsed(page: &str, parsed: &Page, options: &ExampleOptions) -> Vec<Attestation> {
    Planner { page, parsed, seq: 0 }.plan(options)
}

/// The claim a directive defends: the sentence it sits in, in its paragraph, heading or list
/// item (the sentences before it are other claims); when it has no prose before it, or sits
/// in a table cell, the paragraph or heading just before its block; failing that, a cell's
/// own text.
fn comment_claim(page: &Page, comment: &Comment) -> String {
    let lead = own_sentence(&comment.lead);
    let claim = match comment.table {
        None if !lead.is_empty() => lead,
        _ => prose_before(page, comment.block).unwrap_or(lead),
    };
    claim.to_string()
}

/// The last sentence of `lead`: what follows the last sentence end that more prose follows.
fn own_sentence(lead: &str) -> &str {
    let start = sentence_ends(lead).filter(|&end| !trim(&lead[end..]).is_empty()).last().unwrap_or(0);
    trim(&lead[start..])
}

/// The paragraph or heading just before a block in its own container (`Block::previous`):
/// what a fenced block, or a directive on its own line, defends. A block that holds only
/// comments is invisible to a reader, so the paragraph before it is the one that counts.
fn prose_before(page: &Page, block: usize) -> Option<&str> {
    let mut previous = page.flow.get(block)?.previous?;
    while page.is_only_comments(previous) {
        previous = page.flow.get(previous)?.previous?;
    }
    match &page.flow.get(previous)?.kind {
        BlockKind::Paragraph { text } | BlockKind::Heading { text } => Some(text),
        _ => None,
    }
}

struct Planner<'a> {
    page: &'a str,
    parsed: &'a Page,
    seq: usize,
}

struct Base {
    id: String,
    line: usize,
    claim: String,
}

impl Planner<'_> {
    fn plan(&mut self, options: &ExampleOptions) -> Vec<Attestation> {
        let found: Vec<(&Comment, Directive)> = self
            .parsed
            .comments
            .iter()
            .filter_map(|c| parse_directive(&c.body, c.line).map(|d| (c, d)))
            .collect();
        let is_table_cases =
            |c: &Comment, d: &Directive| matches!(d, Directive::Cases { .. }) && c.table.is_none();
        let mut bound: HashSet<usize> = HashSet::new();
        // Table cases are planned first: the tables they bind are skipped by the directives
        // below. The result still lists examples, then cases, then the other directives.
        let mut cases = Vec::new();
        for (comment, directive) in &found {
            if let Directive::Cases { runner } = directive
                && is_table_cases(comment, directive)
            {
                cases.push(self.table_cases(comment, runner, &mut bound));
            }
        }
        let in_bound = |t: Option<usize>| t.is_some_and(|t| bound.contains(&t));
        let examples = self.examples(options);
        let mut attestations = examples;
        attestations.extend(cases);
        for (comment, directive) in &found {
            if is_table_cases(comment, directive) || in_bound(comment.table) {
                continue;
            }
            let a = self.directive(comment, directive);
            attestations.push(a);
        }
        for u in &self.parsed.unclosed {
            if !in_bound(u.table) {
                let a = self.unclosed(u);
                attestations.push(a);
            }
        }
        attestations.sort_by_key(|a| a.line);
        attestations
    }

    fn base(&mut self, line: usize, kind: &str, claim: String) -> Base {
        let id = format!("{}:{line}:{kind}:{}", self.page, self.seq);
        self.seq += 1;
        Base { id, line, claim }
    }

    fn attestation(&self, base: Base, body: Body, error: Option<String>) -> Attestation {
        Attestation {
            id: base.id,
            page: self.page.to_string(),
            line: base.line,
            claim: base.claim,
            error,
            body,
        }
    }

    fn examples(&mut self, options: &ExampleOptions) -> Vec<Attestation> {
        let mut out = Vec::new();
        for code in &self.parsed.code {
            let Some(block) = self.parsed.flow.get(code.block) else { continue };
            let BlockKind::Code { info, value } = &block.kind else { continue };
            let base = self.base(
                block.start,
                "example",
                prose_before(self.parsed, code.block).unwrap_or_default().to_string(),
            );
            let planned = plan_core_example(block.start, info, value, options.ignore_tags).or_else(|| {
                plan_example(block.start, info, value, options).map(|(e, error)| (Body::Example(e), error))
            });
            if let Some((body, error)) = planned {
                out.push(self.attestation(base, body, error));
            }
        }
        out
    }

    fn table_cases(&mut self, comment: &Comment, runner: &str, bound: &mut HashSet<usize>) -> Attestation {
        let base = self.base(comment.line, "cases", comment_claim(self.parsed, comment));
        match bind_cases(comment, self.parsed) {
            Err(error) => self.cases_error(base, runner, error),
            Ok((table, header, rows)) => {
                bound.insert(table);
                let body = Body::Cases(Cases { runner: runner.to_string(), header, rows });
                self.attestation(base, body, None)
            }
        }
    }

    fn directive(&mut self, comment: &Comment, directive: &Directive) -> Attestation {
        let kind = match directive {
            Directive::Invalid { as_kind, .. } => as_kind.as_str(),
            Directive::Fact(_) => "fact",
            Directive::Cases { .. } => "cases",
            Directive::Test(_) => "test",
        };
        let base = self.base(comment.line, kind, comment_claim(self.parsed, comment));
        match directive {
            Directive::Invalid { as_kind, message } => self.plan_error(*as_kind, base, message.clone()),
            Directive::Test(citation) => self.citation(base, citation),
            Directive::Cases { runner } => {
                let why = "a directive in a table cell cannot bind one";
                let error = format!("cases at L{} has no table: {why}", comment.line);
                self.cases_error(base, runner, error)
            }
            Directive::Fact(f) => {
                let body = Body::Fact(Fact {
                    expression: f.expression.clone(),
                    surface: f.surface.clone(),
                    ordered: f.ordered,
                    column: f.column.clone(),
                    target: bind_fact(f.ordered, f.column.as_deref(), comment, self.parsed),
                });
                self.attestation(base, body, None)
            }
        }
    }

    fn unclosed(&mut self, u: &Unclosed) -> Attestation {
        let claim = prose_before(self.parsed, u.block).unwrap_or_default().to_string();
        let base = self.base(u.line, u.keyword.as_str(), claim);
        let error = format!("directive at L{} is not closed: end it with -->", u.line);
        self.plan_error(u.keyword, base, error)
    }

    fn plan_error(&self, kind: Keyword, base: Base, error: String) -> Attestation {
        match kind {
            Keyword::Cases => self.cases_error(base, "", error),
            Keyword::Test => {
                let body =
                    Body::Test(Test { file: String::new(), title: String::new(), describes: Vec::new() });
                self.attestation(base, body, Some(error))
            }
            Keyword::Fact => {
                let body = Body::Fact(Fact {
                    expression: String::new(),
                    surface: DEFAULT_SURFACE.into(),
                    ordered: false,
                    column: None,
                    target: Err(error),
                });
                self.attestation(base, body, None)
            }
        }
    }

    // A citation defends a sentence; one with nothing before it to defend is misplaced.
    fn citation(&self, base: Base, c: &Citation) -> Attestation {
        let error = base.claim.is_empty().then(|| {
            let hint = "put it after the sentence it defends, on the same line";
            format!("test at L{} defends no sentence: {hint}", base.line)
        });
        let body =
            Body::Test(Test { file: c.file.clone(), title: c.title.clone(), describes: c.describes.clone() });
        self.attestation(base, body, error)
    }

    fn cases_error(&self, base: Base, runner: &str, error: String) -> Attestation {
        let body = Body::Cases(Cases { runner: runner.to_string(), header: Vec::new(), rows: Vec::new() });
        self.attestation(base, body, Some(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(text: &str) -> Vec<(usize, String)> {
        let languages = ["ts".to_string()];
        let options = ExampleOptions { languages: &languages, ignore_tags: &[], info_words: &[] };
        let plan = plan_page("p.md", text, &options).unwrap_or_else(|e| panic!("{e}"));
        plan.attestations.into_iter().map(|a| (a.line, a.claim)).collect()
    }

    #[test]
    fn a_directive_after_prose_claims_that_prose() {
        let page =
            "Plain prose.\n\nDoubling `4` gives `8`. <!-- fact: D -->\n\n## Port `80` <!-- fact: P -->";
        assert_eq!(claims(page), vec![(3, "Doubling `4` gives `8`.".into()), (5, "Port `80`".into())]);
    }

    #[test]
    fn a_block_claims_the_paragraph_before_it_in_its_container() {
        let page = "Intro.\n\n- Item one.\n  <!-- test: t.ts > \"x\" -->\n- ```ts check\n  x\n  ```\n\n> Quoted.\n>\n> <!-- fact: K -->\n> - `a`";
        assert_eq!(claims(page), vec![(4, "Item one.".into()), (5, "Intro.".into()), (11, "Quoted.".into())]);
    }

    #[test]
    fn a_fact_in_a_cell_claims_the_prose_before_its_table() {
        let table = "| a |\n|---|\n| `3` <!-- fact: N --> |";
        assert_eq!(claims(&format!("Limits:\n\n{table}")), vec![(5, "Limits:".into())]);
        assert_eq!(claims(table), vec![(3, "`3`".into())]);
    }
}
