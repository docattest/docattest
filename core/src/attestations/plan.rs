//! Finding and binding every attestation on a Markdown page.

use std::collections::{HashMap, HashSet};

use super::alias::Aliases;
use super::bind::{Bound, FactRule, bind_cases, bind_fact};
use super::citation::Citation;
use super::core_examples::plan_core_example;
use super::directives::{Directive, FactDirective, parse_directive};
use super::examples::{ExampleOptions, plan_example};
use super::placement::{Placed, detached_error, place};
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
    if parsed.is_frozen() {
        return Ok(PagePlan { frozen: true, attestations: Vec::new() });
    }
    Ok(PagePlan { frozen: false, attestations: plan_parsed(page, &parsed, options) })
}

/// The attestations of a parsed page that is not frozen, in line order.
pub fn plan_parsed(page: &str, parsed: &Page, options: &ExampleOptions) -> Vec<Attestation> {
    plan_explained(page, parsed, options).attestations
}

/// Why a directive's attestation is what it is: where the directive takes effect and, for a
/// fact that bound a target, the rule that bound it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Why {
    pub placed: Placed,
    pub rule: Option<FactRule>,
}

/// A page's attestations, with what `explain` says of its directives.
#[derive(Debug, Clone)]
pub struct Explained {
    /// In line order, as [`plan_parsed`] plans them.
    pub attestations: Vec<Attestation>,
    /// Why each directive's attestation is what it is, by attestation id. A fenced block,
    /// which attests itself, has no entry.
    pub whys: HashMap<String, Why>,
    /// The page's test file aliases, which no attestation carries.
    pub aliases: Aliases,
}

/// [`plan_parsed`], with why each directive's attestation is what it is.
pub fn plan_explained(page: &str, parsed: &Page, options: &ExampleOptions) -> Explained {
    let mut planner = Planner { page, parsed, seq: 0, aliases: Aliases::default(), whys: HashMap::new() };
    let attestations = planner.plan(options);
    Explained { attestations, whys: planner.whys, aliases: planner.aliases }
}

/// The claim a directive defends, given the comment where it takes effect
/// ([`super::placement`]): the sentence it sits in, in its paragraph, heading or list item
/// (the sentences before it are other claims); when it has no prose before it, or sits in a
/// table cell, the paragraph or heading just before its block; failing that, a cell's own
/// text.
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
    /// The page's test file aliases, which hold on the whole page.
    aliases: Aliases,
    whys: HashMap<String, Why>,
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
        // A directive in a table that cases bind is that table's data, an alias too.
        let found: Vec<&(&Comment, Directive)> =
            found.iter().filter(|(c, d)| !is_table_cases(c, d) && !in_bound(c.table)).collect();
        attestations.extend(self.declare_aliases(&found));
        for (comment, directive) in found {
            let planned = self.directive(comment, directive);
            attestations.extend(planned);
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

    /// A base for a directive's attestation, with why it is what it is.
    fn explained(&mut self, line: usize, kind: &str, claim: &str, why: Why) -> Base {
        let base = self.base(line, kind, claim.to_string());
        self.whys.insert(base.id.clone(), why);
        base
    }

    /// Declare the page's aliases before any citation is planned, since they hold on the
    /// whole page; a name declared again is an error where it is.
    fn declare_aliases(&mut self, found: &[&(&Comment, Directive)]) -> Vec<Attestation> {
        let mut errors = Vec::new();
        for (comment, directive) in found.iter().copied() {
            let Directive::Alias(alias) = directive else { continue };
            if let Err(error) = self.aliases.declare(alias, comment.line) {
                let claim = comment_claim(self.parsed, comment);
                let why = Why { placed: Placed::Written, rule: None };
                let base = self.explained(comment.line, Keyword::Tests.as_str(), &claim, why);
                errors.push(self.plan_error(Keyword::Tests, base, error));
            }
        }
        errors
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
        let why = Why { placed: Placed::Written, rule: None };
        let base = self.explained(comment.line, "cases", &comment_claim(self.parsed, comment), why);
        match bind_cases(comment, self.parsed) {
            Err(error) => self.cases_error(base, runner, error),
            Ok((table, header, rows)) => {
                bound.insert(table);
                let body = Body::Cases(Cases { runner: runner.to_string(), header, rows });
                self.attestation(base, body, None)
            }
        }
    }

    /// A directive's attestations: one, one for each title a citation names, or none for an
    /// alias, which only names a file.
    fn directive(&mut self, comment: &Comment, directive: &Directive) -> Vec<Attestation> {
        let kind = match directive {
            Directive::Invalid { as_kind, .. } => as_kind.as_str(),
            Directive::Fact(_) => "fact",
            Directive::Cases { .. } => "cases",
            Directive::Test(_) => "test",
            Directive::Alias(_) => Keyword::Tests.as_str(),
        };
        let placed = place(self.parsed, comment, directive);
        let effective = placed.comment().unwrap_or(comment);
        let claim = comment_claim(self.parsed, effective);
        let detached = placed.comment().is_none().then(|| detached_error(kind, comment.line));
        let why = Why { placed: placed.placed(), rule: None };
        let base = |planner: &mut Self| planner.explained(comment.line, kind, &claim, why);
        match directive {
            Directive::Test(citation) => {
                let mut cite = |title: &String| {
                    let base = base(self);
                    self.citation(base, citation, title, detached.clone())
                };
                citation.titles.iter().map(&mut cite).collect()
            }
            Directive::Fact(f) => {
                let bound = match detached {
                    Some(error) => Err(error),
                    None => bind_fact(f, comment.line, effective, self.parsed),
                };
                let why = Why { rule: bound.as_ref().ok().map(|bound| bound.rule), ..why };
                let base = self.explained(comment.line, kind, &claim, why);
                vec![self.fact(base, f, bound)]
            }
            Directive::Cases { runner } => {
                let why = "a directive in a table cell cannot bind one";
                let error = format!("cases at L{} has no table: {why}", comment.line);
                let base = base(self);
                vec![self.cases_error(base, runner, error)]
            }
            Directive::Invalid { as_kind, message } => {
                let base = base(self);
                vec![self.plan_error(*as_kind, base, message.clone())]
            }
            Directive::Alias(_) => Vec::new(),
        }
    }

    fn fact(&self, base: Base, f: &FactDirective, bound: Result<Bound, String>) -> Attestation {
        let body = Body::Fact(Fact {
            expression: f.expression.clone(),
            surface: f.surface.clone(),
            ordered: f.ordered,
            column: f.column.clone(),
            field: f.field.as_ref().map(|path| path.written.clone()),
            target: bound.map(|bound| bound.target),
        });
        self.attestation(base, body, None)
    }

    fn unclosed(&mut self, u: &Unclosed) -> Attestation {
        let claim = prose_before(self.parsed, u.block).unwrap_or_default();
        let why = Why { placed: Placed::Written, rule: None };
        let base = self.explained(u.line, u.keyword.as_str(), claim, why);
        let error = format!("directive at L{} is not closed: end it with -->", u.line);
        self.plan_error(u.keyword, base, error)
    }

    fn plan_error(&self, kind: Keyword, base: Base, error: String) -> Attestation {
        match kind {
            Keyword::Cases => self.cases_error(base, "", error),
            Keyword::Test | Keyword::Tests => {
                let body = Body::Test(Test::default());
                self.attestation(base, body, Some(error))
            }
            Keyword::Fact => {
                let body = Body::Fact(Fact {
                    expression: String::new(),
                    surface: DEFAULT_SURFACE.into(),
                    ordered: false,
                    column: None,
                    field: None,
                    target: Err(error),
                });
                self.attestation(base, body, None)
            }
        }
    }

    // A citation defends a sentence; one with nothing before it to defend is misplaced, and
    // so is a detached one. A file that the page aliases is cited by the file it names.
    fn citation(&self, base: Base, c: &Citation, title: &str, detached: Option<String>) -> Attestation {
        let error = detached.or_else(|| {
            base.claim.is_empty().then(|| {
                let hint = "put it at the end of the sentence it defends, or alone on the line above it";
                format!("test at L{} defends no sentence: {hint}", base.line)
            })
        });
        let (file, alias) = match self.aliases.named(&c.file) {
            Some(alias) => (alias.file.clone(), Some(alias.name.clone())),
            None => (c.file.clone(), None),
        };
        let body = Body::Test(Test { file, title: title.to_string(), describes: c.describes.clone(), alias });
        self.attestation(base, body, error)
    }

    fn cases_error(&self, base: Base, runner: &str, error: String) -> Attestation {
        let body = Body::Cases(Cases { runner: runner.to_string(), header: Vec::new(), rows: Vec::new() });
        self.attestation(base, body, Some(error))
    }
}

#[cfg(test)]
mod tests;
