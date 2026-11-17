//! A page that carried attestations at the base and is no longer living: frozen by the config,
//! frozen by its frontmatter, dropped from `pages`, or moved somewhere unchecked.

use std::collections::HashSet;

use super::change::{Change, ChangeType};
use super::side::Side;
use super::signature::proof_of;
use crate::api::errors::{Error, Result};
use crate::attestations::examples::ExampleOptions;
use crate::attestations::plan::plan_page;
use crate::config::matcher;

/// Frozen-page changes, and the base pages they cover (their claims are not judged).
#[derive(Debug, Default)]
pub struct Frozen {
    pub changes: Vec<Change>,
    pub pages: HashSet<String>,
}

impl Frozen {
    fn add(&mut self, base_page: &str, page: &str, what: String) {
        self.pages.insert(base_page.to_string());
        let message = format!("{what}, so its attestations no longer run");
        self.changes.push(Change::warning(ChangeType::FrozenPage, page, 1, message));
    }
}

/// Every base page with attestations that left the living set while its file stayed.
pub fn frozen_pages(base: &Side, head: &Side) -> Result<Frozen> {
    let mut frozen = Frozen::default();
    let mut gone = Vec::new();
    for &page in base.pages() {
        if head.living(page) {
            continue;
        }
        if head.project.files.exists(page) {
            frozen.add(page, page, format!("{page} {}", why(page, head)?));
        } else {
            gone.push(page);
        }
    }
    if !gone.is_empty() {
        moved_out_of_living(&gone, base, head, &mut frozen)?;
    }
    Ok(frozen)
}

fn why(page: &str, head: &Side) -> Result<&'static str> {
    if !head.markdown(page) {
        return Ok(if head.project.is_source(page) && !head.in_sources(page) {
            "is no longer matched by \"sources\""
        } else {
            "is no longer matched by \"pages\""
        });
    }
    if matcher(&head.project.config.frozen)?.is_match(page) {
        return Ok("is frozen by \"frozen\" in the config");
    }
    Ok("is frozen by its frontmatter (docattest: false)")
}

/// `/\.(md|mdx|markdown)$/i`.
fn is_markdown_file(file: &str) -> bool {
    let lower = file.to_ascii_lowercase();
    [".md", ".mdx", ".markdown"].iter().any(|extension| lower.ends_with(extension))
}

// A vanished page whose proofs turn up in a new file that is not living was moved out of
// the checked docs (into a frozen directory, say): freezing by another route.
fn moved_out_of_living(gone: &[&str], base: &Side, head: &Side, frozen: &mut Frozen) -> Result<()> {
    let options = ExampleOptions {
        languages: head.project.language.block_languages(),
        ignore_tags: &head.project.config.ignore_tags,
        info_words: head.project.language.info_words(),
    };
    let fresh = head
        .project
        .files
        .list()
        .into_iter()
        .filter(|f| is_markdown_file(f) && !head.living(f) && !base.project.files.exists(f));
    for file in fresh {
        let text = head.project.files.read(&file).map_err(Error::user)?;
        let plan = plan_page(&file, &unfrozen(&text), &options).map_err(|why| why.on(&file))?;
        let proofs: HashSet<String> = plan.attestations.iter().map(proof_of).collect();
        for &page in gone {
            if frozen.pages.contains(page) || !base.on(page).iter().any(|a| proofs.contains(&proof_of(a))) {
                continue;
            }
            frozen.add(page, &file, format!("{page} moved to {file}, which is not a living page"));
        }
    }
    Ok(())
}

// Frontmatter (`/^---\r?\n[\s\S]*?\r?\n---\r?\n/`) is blanked, not removed, so lines stay
// where they were.
fn unfrozen(text: &str) -> String {
    let Some(open) = text.strip_prefix("---\r\n").or_else(|| text.strip_prefix("---\n")) else {
        return text.to_string();
    };
    let body = text.len() - open.len();
    let mut from = body;
    while let Some(at) = text[from..].find("\n---").map(|k| from + k) {
        let after = &text[at + 4..];
        let fence = after.strip_prefix("\r\n").or_else(|| after.strip_prefix('\n'));
        if let Some(rest) = fence {
            let end = text.len() - rest.len();
            return format!("{}{rest}", "\n".repeat(text[..end].matches('\n').count()));
        }
        from = at + 1;
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_blanked_line_for_line() {
        assert_eq!(unfrozen("---\ndocattest: false\n---\n# A\n"), "\n\n\n# A\n");
        assert_eq!(unfrozen("---\r\na: 1\r\nb: 2\r\n---\r\nx"), "\n\n\n\nx");
        assert_eq!(unfrozen("---\n\n---\nx"), "\n\n\nx");
        assert_eq!(unfrozen("---\na\n----\nb\n---\nx"), "\n\n\n\n\nx");
        // Not frontmatter: no closing fence, an empty one, or not at the start.
        assert_eq!(unfrozen("---\na: 1\n"), "---\na: 1\n");
        assert_eq!(unfrozen("---\n---\nx"), "---\n---\nx");
        assert_eq!(unfrozen("\n---\na\n---\n"), "\n---\na\n---\n");
        assert_eq!(unfrozen("---\na\n---"), "---\na\n---");
    }

    #[test]
    fn markdown_files_by_extension_whatever_the_case() {
        assert!(is_markdown_file("a.md") && is_markdown_file("b/C.MDX") && is_markdown_file("d.Markdown"));
        assert!(!is_markdown_file("a.md.txt") && !is_markdown_file("md"));
    }
}
