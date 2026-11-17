//! One side of a comparison, base or head: its plan, and its pages read as prose.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use super::project::Project;
use super::units::{Located, Unit, claim_piece, claim_unit, doc_comment_units, markdown_units, pieces};
use crate::api::attestation::Attestation;
use crate::api::errors::{Error, Result};
use crate::config::matcher;
use crate::files::glob::GlobMatcher;
use crate::markdown::model::BlockKind;
use crate::markdown::parse::parse_page;

/// A fenced block of a Markdown page, tagged or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    pub start: usize,
    pub info: String,
    pub value: String,
}

/// A page read as prose. Units are referred to by their index here.
#[derive(Debug, Default)]
pub struct PageView<'a> {
    pub units: Vec<Unit>,
    pub code: Vec<CodeBlock>,
    /// Every piece of the prose a claim can be ([`pieces`]) with its unit, in page order.
    pub pieces: Vec<(usize, String)>,
    /// The text of every piece, to ask whether the page says something anywhere.
    pub piece_texts: HashSet<String>,
    /// The piece each attestation on the page claims, for those that claim one.
    pub claims: Vec<(&'a Attestation, (usize, String))>,
}

/// A side over a planned project. Pages are read and parsed lazily, once.
pub struct Side<'a> {
    pub project: Project<'a>,
    by_page: HashMap<&'a str, Vec<&'a Attestation>>,
    /// Pages carrying attestations, in plan order.
    pages: Vec<&'a str>,
    is_markdown: GlobMatcher,
    is_source: GlobMatcher,
    living: HashSet<&'a str>,
    /// The doc comments of the side's source files as prose, by page: asked of the language
    /// once, for both sides, before anything is compared ([`prose_of`]).
    prose: HashMap<String, Vec<Unit>>,
    views: RefCell<HashMap<String, Rc<PageView<'a>>>>,
}

/// The prose of the source files among the pages that carry attestations on either side, for
/// each side, in one request to the language: a source file is read as its doc comments, and
/// only the language can read them.
pub fn prose_of(base: &Project, head: &Project) -> Result<[HashMap<String, Vec<Unit>>; 2]> {
    let pages: BTreeSet<&str> =
        base.attestations.iter().chain(head.attestations).map(|a| a.page.as_str()).collect();
    let mut asked: Vec<(usize, &str, String)> = Vec::new();
    for (side, project) in [base, head].into_iter().enumerate() {
        for (page, text) in source_pages(project, &pages)? {
            asked.push((side, page, text));
        }
    }
    let mut out = [HashMap::new(), HashMap::new()];
    if asked.is_empty() {
        return Ok(out);
    }
    let files: Vec<(&str, &str)> = asked.iter().map(|(_, page, text)| (*page, text.as_str())).collect();
    let answers = head.language.doc_prose(&files)?;
    for ((side, page, _), prose) in asked.iter().zip(answers) {
        out[*side].insert((*page).to_string(), doc_comment_units(prose));
    }
    Ok(out)
}

/// The pages among `pages` that are source files of the project, not Markdown, with their text.
fn source_pages<'p>(project: &Project, pages: &BTreeSet<&'p str>) -> Result<Vec<(&'p str, String)>> {
    let is_markdown = matcher(&project.config.pages)?;
    pages
        .iter()
        .filter(|page| !is_markdown.is_match(page) && project.is_source(page) && project.files.exists(page))
        .map(|page| Ok((*page, project.files.read(page).map_err(Error::user)?)))
        .collect()
}

impl<'a> Side<'a> {
    pub fn new(project: Project<'a>, prose: HashMap<String, Vec<Unit>>) -> Result<Side<'a>> {
        let mut by_page: HashMap<&str, Vec<&Attestation>> = HashMap::new();
        let mut pages = Vec::new();
        for a in project.attestations {
            let list = by_page.entry(a.page.as_str()).or_default();
            if list.is_empty() {
                pages.push(a.page.as_str());
            }
            list.push(a);
        }
        Ok(Side {
            project,
            by_page,
            pages,
            is_markdown: matcher(&project.config.pages)?,
            is_source: matcher(&project.config.sources)?,
            living: project.living.iter().map(String::as_str).collect(),
            prose,
            views: RefCell::new(HashMap::new()),
        })
    }

    /// Attestations on a page, in line order.
    pub fn on(&self, page: &str) -> &[&'a Attestation] {
        self.by_page.get(page).map_or(&[], Vec::as_slice)
    }

    /// Pages carrying attestations.
    pub fn pages(&self) -> &[&'a str] {
        &self.pages
    }

    /// Matched by the config's `pages`, so read as Markdown.
    pub fn markdown(&self, page: &str) -> bool {
        self.is_markdown.is_match(page)
    }

    /// Matched by the config's `sources`.
    pub fn in_sources(&self, page: &str) -> bool {
        self.is_source.is_match(page)
    }

    /// A living page, or a source file that would be one if it held a doc-comment example.
    pub fn living(&self, page: &str) -> bool {
        self.living.contains(page)
            || (!self.markdown(page)
                && self.in_sources(page)
                && self.project.is_source(page)
                && self.project.files.exists(page))
    }

    /// The page's units, fenced blocks, pieces and claims.
    pub fn view(&self, page: &str) -> Result<Rc<PageView<'a>>> {
        if let Some(view) = self.views.borrow().get(page) {
            return Ok(Rc::clone(view));
        }
        let view = Rc::new(self.read(page)?);
        self.views.borrow_mut().insert(page.to_string(), Rc::clone(&view));
        Ok(view)
    }

    fn read(&self, page: &str) -> Result<PageView<'a>> {
        // A page the other side has and this one does not reads as empty.
        let text = if self.project.files.exists(page) {
            self.project.files.read(page).map_err(Error::user)?
        } else {
            String::new()
        };
        let text = || text.clone();
        let mut view = PageView::default();
        if self.markdown(page) {
            let parsed = parse_page(&text()).map_err(|why| why.on(page))?;
            view.units = markdown_units(&parsed);
            view.code = parsed
                .code
                .iter()
                .filter_map(|code| parsed.flow.get(code.block))
                .filter_map(|block| match &block.kind {
                    BlockKind::Code { info, value } => {
                        Some(CodeBlock { start: block.start, info: info.clone(), value: value.clone() })
                    }
                    _ => None,
                })
                .collect();
        } else if let Some(units) = self.prose.get(page) {
            view.units = units.clone();
        }
        view.pieces = (view.units.iter().enumerate())
            .flat_map(|(k, unit)| pieces(unit).into_iter().map(move |piece| (k, piece.to_string())))
            .collect();
        view.piece_texts = view.pieces.iter().map(|(_, piece)| piece.clone()).collect();
        view.claims = self.claims(page, &view.units);
        Ok(view)
    }

    /// The attestations on a page that claim a piece of its prose, with that piece.
    fn claims(&self, page: &str, units: &[Unit]) -> Vec<(&'a Attestation, (usize, String))> {
        let claim = |a: &'a Attestation| {
            let located = claim_unit(a, units)?;
            Some((a, (located.unit, claim_piece(a, &units[located.unit]).to_string())))
        };
        self.on(page).iter().filter_map(|&a| claim(a)).collect()
    }

    /// The prose an attestation defends, on its own page.
    pub fn claim(&self, a: &Attestation) -> Result<Option<(Rc<PageView<'a>>, Located)>> {
        let view = self.view(&a.page)?;
        let located = claim_unit(a, &view.units);
        Ok(located.map(|l| (view, l)))
    }
}
