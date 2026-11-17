//! The project as committed at a ref, read on demand for `check --base`. A missing repository or
//! ref fails closed.

use std::path::{Path, PathBuf};

use super::{Failure, blobs, prefix, run};
use crate::api::errors::{Error, Result};
use crate::config::{CONFIG_FILE, parse_config};
use crate::files::glob::GlobMatcher;
use crate::files::ignored::Ignored;
use crate::text::trim;

/// A base that was found: the commit a ref names, the files the project had there and, when
/// it had a config, that config, which is valid. Everything a user can have got wrong about
/// a base is known once this exists; its files are still to be read.
pub struct Base {
    root: PathBuf,
    /// Project path and blob id of every file the base may read, in git's order.
    blobs: Vec<(String, String)>,
    /// The config's text and what it says planning reads; None when the base had no config.
    config: Option<(String, GlobMatcher)>,
}

/// Find the base. Fails when git or the repository is missing, when the ref names no
/// commit, and when the config committed there is not a valid one.
pub fn find(root: &Path, reference: &str) -> Result<Base> {
    let prefix = repository_prefix(root)?;
    let commit = resolve_ref(root, reference)?;
    let blobs = list_blobs(root, &commit, &prefix)?;
    let config = match blobs.iter().find(|(path, _)| path == CONFIG_FILE) {
        Some((_, sha)) => {
            let text = read_blobs(root, &[sha.as_str()])?.into_iter().next().unwrap_or_default();
            let wanted = needed_by(&text, reference)?;
            Some((text, wanted))
        }
        None => None,
    };
    Ok(Base { root: root.to_path_buf(), blobs, config })
}

impl Base {
    /// The files the base needs, as committed: the config, and the pages and sources it
    /// names, in git's order. A path the language never counts as the project's own (`ignored`)
    /// is not read. Empty when the base had no
    /// config.
    pub fn read(self, ignored: &Ignored) -> Result<Vec<(String, String)>> {
        let Some((config, wanted)) = self.config else {
            return Ok(Vec::new());
        };
        let marked = ignored.marked(self.blobs.iter().map(|(path, _)| path.as_str()));
        let own = |path: &str| !ignored.path(path) && !Ignored::under(path, &marked);
        let needed: Vec<&(String, String)> = self
            .blobs
            .iter()
            .filter(|(path, _)| path == CONFIG_FILE || (wanted.is_match(path) && own(path)))
            .collect();
        let shas: Vec<&str> =
            needed.iter().filter(|(path, _)| path != CONFIG_FILE).map(|(_, sha)| sha.as_str()).collect();
        let mut texts = read_blobs(&self.root, &shas)?.into_iter();
        let mut config = Some(config);
        Ok(needed
            .into_iter()
            .map(|(path, _)| {
                let text = if path == CONFIG_FILE { config.take() } else { texts.next() };
                (path.clone(), text.unwrap_or_default())
            })
            .collect())
    }
}

fn git(root: &Path, args: &[&str]) -> std::result::Result<Vec<u8>, Failure> {
    run(root, args, None)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// The project may sit in a subdirectory of the repository; paths are relative to it.
fn repository_prefix(root: &Path) -> Result<String> {
    prefix(root).map_err(|failure| match failure {
        Failure::Missing => Error::user("a base ref is read with git, which was not found"),
        Failure::Failed(_) => Error::user(format!(
            "a base ref is read from git, but {} is not in a git repository",
            root.display()
        )),
    })
}

fn resolve_ref(root: &Path, reference: &str) -> Result<String> {
    // A ref that starts with a dash would be read by git as an option.
    if trim(reference).is_empty() || reference.starts_with('-') {
        return Err(Error::user(format!("\"{reference}\" is not a git ref")));
    }
    let commit = format!("{reference}^{{commit}}");
    match git(root, &["rev-parse", "--verify", "--quiet", &commit]) {
        Ok(out) => Ok(trim(&text(&out)).to_string()),
        Err(_) => {
            Err(Error::user(format!("unknown git ref \"{reference}\": no such commit in {}", root.display())))
        }
    }
}

/// One `ls-tree -z` entry, `/^(\d+) (\w+) ([0-9a-f]+)\t(.*)$/s`: mode, type, sha, path.
fn entry(line: &str) -> Option<(&str, &str, &str, &str)> {
    let (meta, path) = line.split_once('\t')?;
    let mut parts = meta.split(' ');
    let (mode, kind, sha) = (parts.next()?, parts.next()?, parts.next()?);
    let valid = parts.next().is_none()
        && one_or_more(mode, |c| c.is_ascii_digit())
        && one_or_more(kind, |c| c.is_ascii_alphanumeric() || c == '_')
        && one_or_more(sha, |c| matches!(c, '0'..='9' | 'a'..='f'));
    valid.then_some((mode, kind, sha, path))
}

/// `[…]+`: text of one or more characters, each of them one `class` accepts.
fn one_or_more(text: &str, class: impl Fn(char) -> bool) -> bool {
    !text.is_empty() && text.chars().all(class)
}

// Like the disk source, skip hidden directories; skip links and submodules. What the language
// ignores by name is left out when the files are read ([`Base::read`]).
fn list_blobs(root: &Path, commit: &str, prefix: &str) -> Result<Vec<(String, String)>> {
    let out = git(root, &["ls-tree", "-r", "-z", "--full-tree", commit]).map_err(failed)?;
    Ok(blobs_in(&text(&out), prefix))
}

/// Project path and blob id of every file of the listing the base may read.
fn blobs_in(listing: &str, prefix: &str) -> Vec<(String, String)> {
    let mut blobs = Vec::new();
    for line in listing.split('\0') {
        let Some((mode, kind, sha, full)) = entry(line) else { continue };
        if kind != "blob" || mode == "120000" {
            continue;
        }
        let Some(path) = full.strip_prefix(prefix) else { continue };
        let mut names: Vec<&str> = path.split('/').collect();
        names.pop();
        if names.iter().any(|d| d.starts_with('.')) {
            continue;
        }
        blobs.push((path.to_string(), sha.to_string()));
    }
    blobs
}

/// The contents of blobs, by id, in order.
fn read_blobs(root: &Path, shas: &[&str]) -> Result<Vec<String>> {
    let objects: Vec<String> = shas.iter().map(|sha| (*sha).to_string()).collect();
    let mut found = blobs(root, &objects).map_err(failed)?.into_iter();
    shas.iter()
        .map(|sha| {
            found
                .next()
                .flatten()
                .ok_or_else(|| Error::internal(format!("git cat-file returned no blob for {sha}")))
        })
        .collect()
}

// The base has been found by now, so git failing to list or read it is unexpected.
fn failed(failure: Failure) -> Error {
    let why = match failure {
        Failure::Missing => "git was not found".to_string(),
        Failure::Failed(why) => why.trim().to_string(),
    };
    Error::Internal { message: "git failed to read the base".to_string(), detail: why }
}

// Only what planning reads: the base config decides which pages and sources those are.
fn needed_by(config_text: &str, reference: &str) -> Result<GlobMatcher> {
    let unreadable =
        |message: &str| Error::user(format!("the base \"{reference}\" cannot be read: {message}"));
    let config = parse_config(config_text).map_err(|e| unreadable(&unprefixed(&e)))?;
    let globs: Vec<String> = config.pages.iter().chain(&config.sources).cloned().collect();
    GlobMatcher::new(&globs).map_err(|message| unreadable(&message))
}

/// An error's message without the `docattest: ` a user error starts with.
pub fn unprefixed(error: &Error) -> String {
    crate::api::errors::unprefixed(&error.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Temp, repository as committed};

    /// The files of the base at `reference`: [`find`], then [`Base::read`].
    fn read_git_base(root: &Path, reference: &str, ignored: &Ignored) -> Result<Vec<(String, String)>> {
        find(root, reference)?.read(ignored)
    }

    const CONFIG: &str = r#"{"pages":["docs/**/*.md"],"sources":["src/**/*.ts"],"surfaces":{"default":"./src/index.ts"},"adapter":"typescript"}"#;

    /// Names a language might ignore.
    fn ignored() -> Ignored {
        Ignored::new(&["deps".to_string()], &[]).unwrap()
    }

    fn temp_dir() -> Temp {
        Temp::new("plain")
    }

    fn write(root: &Path, files: &[(&str, &str)]) {
        for (path, text) in files {
            let target = root.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, text).unwrap();
        }
    }

    fn pairs(files: &[(&str, &str)]) -> Vec<(String, String)> {
        files.iter().map(|(path, text)| (path.to_string(), text.to_string())).collect()
    }

    fn message(result: Result<Vec<(String, String)>>) -> String {
        result.map(|files| format!("{files:?}")).unwrap_or_else(|error| error.to_string())
    }

    #[test]
    fn reads_the_config_pages_and_sources_as_committed_nothing_else() {
        let root = committed(&[
            ("docattest.config.json", CONFIG),
            ("package.json", r#"{"name":"x"}"#),
            ("docs/a.md", "committed"),
            ("src/index.ts", "export const N = 1;"),
            ("notes/other.md", "not a page"),
            (".hidden/docs/b.md", "hidden"),
            ("docs/deps/x/readme.md", "ignored by name"),
            ("src/deps/a.ts", "ignored by name"),
        ]);
        write(&root.0, &[("docs/a.md", "edited in the working tree"), ("docs/new.md", "untracked")]);
        let mut files = read_git_base(&root.0, "HEAD", &ignored()).unwrap();
        files.sort();
        assert_eq!(
            files,
            pairs(&[
                ("docattest.config.json", CONFIG),
                ("docs/a.md", "committed"),
                ("src/index.ts", "export const N = 1;"),
            ])
        );
        // Without the names, the same listing keeps what they would have left out.
        assert_eq!(read_git_base(&root.0, "HEAD", &Ignored::default()).unwrap().len(), 5);
    }

    #[test]
    fn reads_a_project_in_a_subdirectory_of_the_repository_relative_to_that_directory() {
        let repo =
            committed(&[("pkg/docattest.config.json", CONFIG), ("pkg/docs/a.md", "A"), ("docs/b.md", "B")]);
        assert_eq!(
            read_git_base(&repo.0.join("pkg"), "HEAD", &ignored()).unwrap(),
            pairs(&[("docattest.config.json", CONFIG), ("docs/a.md", "A")])
        );
    }

    #[test]
    fn has_nothing_when_the_base_had_no_config_and_fails_closed_otherwise() {
        let root = committed(&[("README.md", "before docattest")]);
        assert_eq!(read_git_base(&root.0, "HEAD", &ignored()).unwrap(), Vec::new());
        let at = root.0.display();
        assert_eq!(
            message(read_git_base(&root.0, "no-such-ref", &ignored())),
            format!("docattest: unknown git ref \"no-such-ref\": no such commit in {at}")
        );
        assert_eq!(
            message(read_git_base(&root.0, "--output=x", &ignored())),
            "docattest: \"--output=x\" is not a git ref"
        );
        assert_eq!(message(read_git_base(&root.0, " ", &ignored())), "docattest: \" \" is not a git ref");
        let plain = temp_dir();
        assert_eq!(
            message(read_git_base(&plain.0, "HEAD", &ignored())),
            format!(
                "docattest: a base ref is read from git, but {} is not in a git repository",
                plain.0.display()
            )
        );
        // Outside a repository that is said first, whatever the ref.
        assert!(message(read_git_base(&plain.0, "-x", &ignored())).ends_with("is not in a git repository"));
        let broken = committed(&[("docattest.config.json", "{ nope")]);
        let said = message(read_git_base(&broken.0, "HEAD", &ignored()));
        assert!(
            said.starts_with(
                "docattest: the base \"HEAD\" cannot be read: docattest.config.json is not valid JSON: "
            ),
            "{said}"
        );
    }

    #[test]
    fn many_blobs_are_read_in_one_batch() {
        let pages: Vec<(String, String)> = (0..3000)
            .map(|k| (format!("docs/p{k}.md"), format!("# Page {k}\n{}", "prose ".repeat(40))))
            .collect();
        let mut files: Vec<(&str, &str)> = pages.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        files.push(("docattest.config.json", CONFIG));
        let root = committed(&files);
        let read = read_git_base(&root.0, "main", &ignored()).unwrap();
        assert_eq!(read.len(), 3001);
        assert!(pages.iter().all(|page| read.contains(page)));
    }

    #[test]
    fn a_listing_keeps_blobs_under_the_prefix_outside_hidden_directories() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let listing = [
            format!("100644 blob {sha}\tpkg/docs/a.md"),
            format!("100755 blob {sha}\tpkg/src/x y.ts"),
            format!("120000 blob {sha}\tpkg/docs/link.md"),
            format!("160000 commit {sha}\tpkg/vendor/sub"),
            format!("100644 blob {sha}\tpkg/.hidden/b.md"),
            format!("100644 blob {sha}\tpkg/docs/.dot/b.md"),
            format!("100644 blob {sha}\tpkg/node_modules/x/readme.md"),
            format!("100644 blob {sha}\tpkg/a/node_modules"),
            format!("100644 blob {sha}\tpkg/.editorconfig"),
            format!("100644 blob {sha}\tother/docs/c.md"),
            format!("100644 blob {sha}\tpkg/docs/tab\there.md"),
            "garbage".to_string(),
            String::new(),
        ]
        .join("\0");
        let paths: Vec<String> = blobs_in(&listing, "pkg/").into_iter().map(|(path, _)| path).collect();
        assert_eq!(
            paths,
            vec![
                "docs/a.md",
                "src/x y.ts",
                "node_modules/x/readme.md",
                "a/node_modules",
                ".editorconfig",
                "docs/tab\there.md"
            ]
        );
        assert_eq!(blobs_in(&listing, "").len(), 7);
    }

    #[test]
    fn a_config_that_cannot_be_read_names_the_base() {
        let Err(error) = needed_by("{ nope", "HEAD") else { panic!("expected an error") };
        assert!(error.to_string().starts_with(
            "docattest: the base \"HEAD\" cannot be read: docattest.config.json is not valid JSON: "
        ));
        let Err(error) = needed_by("{}", "v1") else { panic!("expected an error") };
        assert!(
            error
                .to_string()
                .starts_with("docattest: the base \"v1\" cannot be read: invalid docattest.config.json: ")
        );
    }
}
