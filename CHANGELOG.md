# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [0.1.4] - 2026-11-17

### Added

- `check --report junit=<file>` and `--report codequality=<file>` also write the report as
  JUnit XML and as a GitLab Code Quality report, with paths from the repository's root; the
  README shows a GitLab CI job that uses them.
- `manifest()` reads YAML (`.yaml`, `.yml`), such as `pnpm-workspace.yaml` or a CI file.
- Warnings, which never change the exit code: a `pages` or `sources` glob that matches no
  file, a `setup` module that is not there or that nothing reads, and (TypeScript) a
  workspace package a surface imports whose build is missing or older than its sources.
- **Rust**: a crate that declares a module of its own name (an Anchor program) can be a
  surface; a surface that names a private module says which module is private; a fact that
  reaches an item by its crate path names it for `where` and `impact`; and
  `DOCATTEST_CARGO_FEATURES` enables features of the crate built for facts and cases.

### Fixed

- `check --base` reports a removed fact whose sentence stays on the page even when other
  proofs remain in its paragraph, and quotes that sentence rather than the paragraph's first.
- A `!` glob in `pages` (or another glob list) is a configuration error pointing to `frozen`,
  instead of making every file a page; an internal error names the page it was reading.
- Files git ignores are neither pages nor sources.
- `[...manifest("x.json")]`: a spread before `manifest` is not mistaken for member access.
- A template that fills in a value (`` manifest(`${dir}/x.json`) ``) is reported as built at
  run time, not read as a literal path.
- An error from `inventory` names `inventory`, not `audit`.
- **Go**: a cases row names the symbols its runner reaches through the setup package's
  helpers, so `where` finds it.

### Changed

- With pages named, `check` counts the uncovered pages instead of listing the whole project's;
  frozen pages are counted, never listed, in the text report; `--json` keeps both lists.
- A renderer's word on an untagged block (` ```ts twoslash `) leaves it documentation; only a
  likely misspelling of `check`, `compile` or `network` is an error, naming the tag.
- A fact that ends a paragraph with nothing to compare binds the list or fenced block after
  it across blank lines (`The keys are: <!-- fact: … -->`, a blank line, a list).
- A number or version may be written with a leading `v`: `v1.2.3` compares as `1.2.3`.
- A column fact compares every code span of a cell (`` `A`, `B` ``), not only the first.
- A set failure leads with what differs and gives the counts only when they differ:
  `missing: C (2 items on the page, 3 in KEYS)`.
- A fenced block compares token by token, so `[1,2]` matches `[1, 2]`.
- `column=route path` says how to quote a column name with spaces.
- Column facts stacked above one table, one per column, each bind it.
- The surface summary counts symbols "not attested by any page" (it said "not referenced",
  though a page may name a symbol it does not attest).

## [0.1.3] - 2026-11-17

### Added

- **GitHub Action**: `uses: docattest/docattest@v0.1.3` runs `docattest check` and marks each
  claim that fails on its line of the pull request, with the counts and the failures on the
  run's summary page. It runs the docattest your project installed, in any language.

### Changed

- The npm, PyPI and crates.io pages link to the website and its playground.

## [0.1.2] - 2026-11-17

### Added

- **Rust**: `cargo install docattest` installs the command from crates.io. The core is the
  crate `docattest-core`, and its patched markdown parser `docattest-markdown`.
- **Go**: `go install github.com/docattest/docattest/adapters/go/cmd/...@latest` brings a
  working docattest, the core for your platform included, with nothing else to download.

## [0.1.1] - 2026-11-17

### Changed

- The npm packages are published by npm's trusted publishing (OIDC) from the release
  workflow, with provenance, and no token is stored anywhere.

## [0.1.0] - 2026-11-16

First public release.

### Proofs on a page

- **Examples**: code blocks tagged `check` run, `compile` blocks are type-checked (TypeScript),
  checked for syntax, imports and names (Python) or compiled (Go, Rust). Each runs isolated, with
  a time limit, with the network off unless a `check network` block runs with
  `DOCATTEST_NETWORK=1`. A block that asserts nothing counts as weak.
- **Commands and sample pages**, run by docattest itself in any project: a `console check`
  block is commands (`$ …`) and what they print, each run with `sh -c` and compared line by
  line (`...` for any lines, `[exit N]` for a command that fails); an `md check
  fixture=<dir>` block is a sample page, checked inside a temporary copy of the fixture
  project, with the adapter the checked project or one of its `members` uses for the
  fixture's language. Both are reported as examples.
- **Facts**: `<!-- fact: expression -->` binds a value (a code span, a number or a version),
  the code spans of a sentence, a list, a table column or a fenced block to an expression
  over the project's public API; a failure names the missing and unexpected values, or the
  first line of a block that differs. `snapshot("name")` reads pinned data about an outside
  system, refetched by `docattest refresh`; `manifest("path")` reads a project file (JSON,
  TOML or `go.mod`) such as `package.json`, so version and packaging claims need no setup
  code.
- **Cases**: `<!-- cases: runner -->` runs a table row by row through a runner in the setup
  module, which may also provide named facts.
- **Citations**: `<!-- test: file > "title" -->` binds a sentence to a test that must exist.
- **Doc comments**: `@example` (TypeScript), `>>>` sessions in docstrings (Python),
  testable `Example` functions (Go) and the fenced blocks of doc comments (Rust) in the
  files under `sources` are checked like pages.
- Pages are adopted one at a time: a page with no proofs is reported as uncovered, never
  failed, and `frozen` pages are listed and never read.

### Commands

- `check [pages…]`, with `--json` and `--quiet`; exit 1 when a proof fails or the project is
  misconfigured, 2 for a bug in docattest. `--quiet` prints only what needs attention
  (failures, errors, surface warnings, stale snapshots, changes since the base) and the
  verdict, so a project where everything holds prints `ok`.
- `check --base <ref>` also fails when a claim stayed on the page but its proof was removed,
  and warns when a page was frozen, a `check` block was downgraded or a fact was weakened.
- `where`, `uncovered` and `index`: from a symbol to the sentences that describe it (the
  prose a directive ends, or the paragraph just before a block in its own list item, quote
  or table), and what no page describes yet.
- `audit` finds lists, table columns, blocks and values in sentences (an enumeration of code
  spans, or one distinctive value) that look copied from the code and carry no proof, with
  the directive to add; `inventory` lists the values a fact could read, the setup module's
  named facts that take no arguments included.
- `impact [--base <ref>]` and `impact --file <path>`: the documented symbols a change affects
  and the prose that names them, the facts that read a manifest it changed, and the console
  and sample blocks that run a command built from a changed file (`commands` in the config).
  A sentence defended by a cited test is listed as cited, not proven. `--file` answers for
  that file's change alone, reads a relative path from the working directory, and a path
  outside the project, or one that is neither there nor in the diff, is an error (exit 1),
  never a quiet "nothing affected".
- `members` in a config: one `check` at the top of a repository checks every project in it,
  each with its own config and adapter, and fails when any of them fails.
- `hooks install [--agent <name or file>]`, `hooks install --git`, `agents install` and
  `skill install`: run `impact` after an agent's edits and `check` before it stops, through
  the agent's own hooks, an `AGENTS.md` section, or a git pre-commit hook. Coding agents are
  described by data: Claude Code (the default), Gemini CLI and Cursor ship, and
  `--agent ./file.json` takes a descriptor of your own.
- `createProject` (npm) runs the same commands from code and returns their JSON, typed.

### Languages and packages

- One core, written in Rust, holds every rule; a language adapter answers only what needs
  the language, over a documented protocol with JSON Schemas and a conformance suite that
  any adapter can be held to.
- TypeScript and JavaScript: the npm package `docattest` (Node 22.12+; TypeScript from 5.4,
  below 7, as a peer dependency).
- Python: the PyPI package `docattest` (Python 3.10+, no dependencies, no Node needed).
- Go: a release archive per platform with the `docattest` command, the Go adapter and the
  core (your project's `go` 1.21+, or 1.23+ on recent macOS; no dependencies, no Node or
  Python needed). A Go block is statements, declarations or a whole program, proven by an
  `// Output:` comment or by not panicking; facts are Go expressions over a package's
  exports.
- Rust: `cargo install` builds the `docattest` command with the core inside it, and the
  Rust adapter (your project's `cargo`, nothing else). A Rust block is read as a doc test
  is, proven by `assert!` and its kin; facts are Rust expressions over a module's exports,
  shown by `Display`, else `Debug`. The core checks its own README with it. The exports a
  macro writes are asked of the compiler (rustdoc's JSON).
- Every package brings the core for the machine, with no install script and no download at
  run time: macOS arm64 and x64, Linux x64 and arm64 (static builds, for glibc and musl), and
  Windows x64.

[0.1.2]: https://github.com/docattest/docattest/releases/tag/v0.1.2
[0.1.1]: https://github.com/docattest/docattest/releases/tag/v0.1.1
[0.1.0]: https://github.com/docattest/docattest/releases/tag/v0.1.0
