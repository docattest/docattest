# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [0.5.0] - 2026-11-22

### Added

- `docattest init`: detects language, pages and default surface, writes `docattest.config.json`
  (with `$schema`), runs an initial gap audit by default (`--no-audit` or `--dry-run` to skip),
  and wires integrations in one command via `--agents`, `--hooks [--local]`, `--git-hook`, and
  `--skill [--global] [--force]`.
- `docattest audit --apply` (`--include-drift`, `--dry-run`): inserts suggested directives
  directly into `.md` and `.mdx` pages.
- Core-only mode (`"adapter": "none"`, the default when `adapter` is omitted) and
  surface-optional configs: check manifests, files, snapshots, `console check` blocks, citations,
  or standalone examples without a language runtime or exported surface.
- MDX support (`.mdx` pages with `{/* fact: ... */}` JSX comment directives and `md check`
  preservation).
- Published JSON Schema for `docattest.config.json` (`protocol/schema/config.json`, hosted at
  `https://docattest.github.io/docattest/schema/config.json`).
- Internal surfaces (`{ "path": "<file>", "internal": true }`): usable by `@name` facts,
  `audit`, `impact`, and `where` while excluded from public API coverage in `check` and
  `uncovered`.
- Unified `Output:` / `Unordered output:` comment assertions on `check` blocks across
  TypeScript, Python, Rust, and Go.
- `docattest check --member <dir> [pages…]`: check a single configured member project (including
  nested members) directly from a monorepo root using its configured `adapterCommand`.
- Bottom-of-report `failures:` recap in mixed single-project and multi-project failing `check`
  runs so failing `path:line` entries are not buried in long output.

### Changed

- Consolidated the CLI from 15 commands down to 9 (`init`, `check`, `audit`, `impact`,
  `explain`, `where`, `uncovered`, `refresh`, `conformance`):
  - `index` merged into `where` (no argument prints the full symbol → references map).
  - `inventory` merged into `audit --inventory [text]`.
  - `agents install`, `hooks install [--local] [--git]`, and `skill install [--global] [--force]`
    absorbed into `init`.
- `init --skill` now installs a lean, single-language `SKILL.md` tailored to the project's
  `adapter` (~55 lines) instead of a multi-language reference, and `AGENTS.md` / `SKILL.md`
  recommend `check --quiet`, targeted `<page>` / `--member`, and `<!-- test: ... -->` citations
  for heavy integration scenarios.
- Config validation now rejects `surfaces` when `adapter` is omitted or `"none"`.

### Removed

- Removed `docattest mutate`, the `mutants` adapter capability/RPC, and mutation testing code
  across the core, protocol, conformance suite, and all four language adapters.

## [0.4.1] - 2026-11-21

### Added

- `docattest hooks install --local` checks `git check-ignore` inside a git repository and
  prints a reminder when the local settings file is not yet ignored by git.
- Citation relevance warnings and citation `symbols` now include `package main` / entrypoint
  symbols and non-exported constants readable from source (in Go, Python and Rust) without
  exposing them to `uncovered`.

## [0.4.0] - 2026-11-21

### Added

- `docattest mutate --base <ref>` (and `project.mutate({ base })`) mutates only the proofs
  `impact --base <ref>` says the diff affects plus proofs added or changed on Markdown pages
  since `<ref>`, making `mutate --fail-on-survivors` cheap on merge requests.
- `docattest mutate` now mutates `console check` blocks in memory against their captured
  output (catching vacuous `...`-only blocks as survivors and reporting how many lines `...`
  hid as `hiddenLines` without failing `--fail-on-survivors`), mutates non-literal scalar
  constants via type-guided expression edits across all four adapters (such as
  `const Period = time.Second` → `2 * time.Second`), and prints an actionable hint suggesting
  `matched()` or `manifestField()` when a whole-file `file()` or `manifest()` fact survives.
- Citation relevance warnings: `symbols` for citations now follows local test helpers (and
  enclosing table-driven test setup in Go/TypeScript) across all four adapters, and `check`
  warns when a passing citation defends a sentence whose code spans name exported surface
  symbols that the cited test does not use.
- `docattest hooks install --local` writes hooks to an agent's untracked project-local
  settings file (`settings.localFile`, such as `.claude/settings.local.json` for Claude
  Code), and shipped agent descriptors for `codex` and `antigravity` join `claude`, `cursor`
  and `gemini`.
- `check` warns when an installed project-local `SKILL.md` is out of date with the running
  docattest, and Go's version-mismatch warning prints the exact copy-pasteable `go get` and
  `go mod tidy` command.
- Go: `check` now vets a separate setup module (`go vet -tags docattest ./...`) and reports
  any findings as failing `problem` results, alongside docs helper files.
- Go: `manifestField("path", "keys")` is typed at call sites in generated fact programs to
  the field's concrete Go type (`string`, `int64`, `float64`, `bool`, `[]any`,
  `map[string]any`), so string concatenation and arithmetic need no type assertion while
  existing `.(T)` assertions keep compiling.

## [0.3.0] - 2026-11-19

### Added

- `docattest check --run-cited` runs the test each citation names, after the check, and a
  citation then holds only when its test ran and passed: a test that fails fails its
  citation with what the runner said, and one that is skipped or that the runner does not
  run is an error that says why. Each adapter runs the project's own runner with the test
  selected exactly and reads the runner's own report, one run of the runner for each file,
  package or target, `runtime.workers` at a time. TypeScript: Vitest (`--reporter=json`), Jest
  (`--json`) or node:test (with a reporter the package ships), as the file imports or the
  nearest `package.json` declares, by an anchored pattern over the test's full name and read
  by its exact path. Python: pytest by node id (`file::Class::test[id]`, JUnit report) when
  the project's interpreter has it, else unittest through the adapter's driver, which
  imports the file as `unittest discover` does; a `subTest` case runs with its test and ends
  as its subtest did. Go: `go test -json -count=1 -run '^TestX$/^case$'` in the file's
  package, subtest names rewritten as `testing` rewrites them, read from test2json; every
  cited test of a package that does not build fails with the build errors. Rust: `cargo test`
  lists the target that owns the file (`--lib`, `--test`, `--bin`, `--bench`, `--example`)
  and runs the names it listed with `--exact`, so rstest and test-case cases run by the
  names their macros give them; the outcome is read from libtest's text. The coverage line
  says how many citations ran (`and 3 citations, 3 of them run`), and `--json` gives
  `coverage.citedRun`, both only when cited tests ran: a citation that ran is still not a
  proof, since a run shows that the test passes, not that it tests the sentence. A run of a
  runner may take ten minutes. It is a flag and not a config key, so the cost falls on the
  check that asks for it (CI) and not on the check after each edit. The protocol gains the
  optional method `runTests`, listed in `hello.capabilities`; without it, `--run-cited` is
  a user error, and the conformance suite checks that a project in memory runs nothing.
- `docattest mutate [pages…]` (and `project.mutate({ pages })`): how strong each proof is.
  Every proof that passes is run again against changes to what it reads, one at a time: the
  literals of the constants it reads (in copies of the project beside it, never in the
  project), each line of a file, what a regex matched and each value of a manifest a fact
  reads, and each cell of a table's row but the first. It reports each proof as killed (by
  the first change that made it fail), survived (with every change it survived) or not
  mutated (with why: a function's result, a value with no literal, a citation, a proof that
  does not pass), and for a row the cells no change of which failed it. A change that leaves
  the code unable to build or load (an error, or an example's compile diagnostics, pointing at
  a line of the changed file, or a project that does not load) is stillborn: it is counted
  and kills nothing. The copies are removed at the end, on Ctrl-C and SIGTERM too, and a run
  removes those a killed run left. `--json`; exit `0`,
  or `1` with `--fail-on-survivors` when a proof survived. Code mutants come from a new
  optional adapter method, `mutants` (capability), which every adapter implements: edits to
  the literals of a symbol's declaration, spelled as the language writes them (a number plus
  one, a string with an `x` before its closing quote, a boolean flipped, a list without its
  last item), at most 8 a symbol; the conformance suite checks it. The agent skill suggests
  running it on a new page.
- `matched("<path>", "<regex>")` and `manifestField("<path>", "<keys>")` in fact
  expressions, in every language: one value of a project file in one call, worked out by
  the core so that every language gets the same value. `matched` is the text of the
  regex's first group in its first match (the whole match when it has no group), with `^`
  and `$` at the ends of each line and `\r\n` read as `\n`, in the core's regex syntax;
  `manifestField` is the value at a path of keys of a manifest, written as `field=` writes
  one, in the language's own kind of value, so a deep field needs no type assertions in
  Go. No match, an invalid regex and a path that is not there are each an error at the
  fact that says what is there instead. A changed file reaches `impact` through them as
  through `file` and `manifest`. `open` carries what they give as `matches` and
  `manifestFields`, each absent when no fact makes such a call, and the conformance suite
  checks both.
- A call's arguments may be raw string literals (`r"…"`, `r'…'`, `r#"…"#`, `` `…` ``), and a
  quoted one may escape a backslash and its own quote; the core reads no other escape, since
  each language reads it differently. This holds for `manifest` and `file` too.
- `docattest explain <page>:<line>`, or `<page>` for every directive on it, and
  `project.explain(page, line)`: for each directive, what it binds and the rule that bound
  it (a `column=` or `field=`, the list or block below it, the code span or number before it,
  the code spans of its sentence), where it takes effect (where written, on the sentence
  below it, or nowhere), the target as the page has it, the sentence it defends, and what
  checking it gives: a fact's value and verdict, a citation's file and whether it declares
  the test, a table's rows and their results. `--json` gives the same as an object.
- A citation's last segment may be a set of titles, `> {"python", "node"}`, each cited as if
  written apart, with its own result.
- `<!-- tests: <name> = <file> -->` names a test file for its page, and the page's citations
  may write the name in its place. An alias holds on the whole page; a name is declared once,
  never looks like a path, and must not also be a file of the project.
- Prose pages: `prose` in the config, globs as `frozen` takes, or frontmatter
  `docattest: prose`, marks a living page that has nothing to prove. It is still checked and
  audited, but never reported as uncovered: the report lists it under `coverage.prose`, and
  the coverage line counts it (`1 uncovered, 2 prose, 0 frozen`) in a project that has some.
  A page marked as prose that has an attestation counts as covered, and its marker is a
  warning.
- Go: a setup package can be a module of its own: a `docs/go.mod` that requires your module,
  with a `replace` to the project root, so your product's `go.mod` requires nothing only the
  pages need, such as `github.com/docattest/docattest/adapters/go` for `cells`. The adapter
  runs the go command in a workspace of the two modules, a `go.work` it writes into its own
  temporary directory, built from the project's own `go.work` when it has one (its `go`
  line, modules and `replace` directives, with the setup module added) unless `GOWORK=off`;
  nothing is written into the project.
- Go: `check` runs `go vet -tags docattest` on each surface package that has docs helper
  files and reports what vet finds in those files, which a plain `go vet ./...` never vets,
  as a failure at the file and line (a `problem` result), so it fails the check. The Go
  guide shows how to give the tag to `go vet` in CI and to golangci-lint
  (`run.build-tags`).
- Go: `check` warns when your `go.mod`, or the setup module's, requires
  `github.com/docattest/docattest/adapters/go` at another release than the docattest that
  runs, naming the file and both versions; a `replace` of the module or a pseudo-version is
  not compared. The warning is about the whole project, listed with the config's warnings.
- Protocol: an optional capability `problems`, which the core asks once per `check`. Its
  answer holds problems, failures that belong to no attestation (such as what a linter
  finds in a file only docattest builds), each at a project file and line, and warnings
  about the whole project. The core reports each problem as a `fail` result of the new kind
  `problem` at its file and line, in the text, `--json`, JUnit and Code Quality reports and
  the GitHub Action's annotations, whether or not the file is a page, and also in a check
  of named pages. The results line counts it, page coverage does not, and it fails the exit
  code. The warnings are listed with the config's and never fail. The conformance suite
  checks that an adapter that lists the capability answers each problem at a path inside
  the project; one that does not list it is asked nothing new.

### Changed

- Rust: the adapter generates and builds its crate in `target/docattest/<a hash of the
  workspace root>`, so that two copies of a project that share one `target` (as `mutate`'s
  copies do) never write the same generated crate.
- Python: the processes that run a project's code write no bytecode
  (`PYTHONDONTWRITEBYTECODE`), so a run never writes into the project, or into a directory a
  copy links to.
- Go: the adapter imports `golang.org/x/mod` directly (it was already required through
  `golang.org/x/tools`), to read `go.mod` and `go.work` files.

## [0.2.2] - 2026-11-19

### Fixed

- A value holds the values of the names its initializer uses, so a fact that reads a constant
  or a variable, directly, through a setup or docs helper, or in a cases runner, attests those
  names too, at any depth, in every language: a fact over `ENV`, built from `COMMAND_PATH`,
  attests `COMMAND_PATH`, so `where` lists the fact, `uncovered` no longer lists the name, and
  `impact` lists the fact when it changes. A private constant used there is credited for
  `where` and never listed by `uncovered`. A function's body is not followed: a fact over
  `total()` attests `total`, not what its body reads. A value only re-exported from another
  module is not followed in TypeScript and Python.
- A fact that calls a docs helper attests the private constants and literal variables the
  helper reads, so `where` lists the fact under them, and the surface's functions the helper
  calls, by name, without following their bodies (Go and Rust).

## [0.2.1] - 2026-11-19

### Added

- Docs helpers: code inside a surface that only docattest builds, so a page can state what a
  helper works out from a private value with no export made for the docs. In Go, a file of
  the surface's package behind `//go:build docattest` (docattest lists packages and builds
  its own programs with that tag, `go build ./...` leaves the file out, and a fact calls what
  it exports); in Rust, a `pub` item of the surface's module under `#[cfg(docattest)]`, built
  with `--cfg docattest` (added to `RUSTFLAGS` when it is set, else to cargo's
  `build.rustflags`). A docs helper is no export, so `uncovered` never lists it, and a fact
  that calls one attests it and what it uses. A page's example cannot call one: Go builds a
  block without the tag, and Rust, which builds every program with the cfg so dependencies
  compile once, refuses a block or doc-comment example that names a docs helper, at that line. In Go a setup
  package behind the tag is now read too.
- Go: a fact reads a literal variable from source, as it reads a constant: a `var` of its
  surface's package, exported or not, of a `package main` too, that its declaration gives a
  composite literal of constants and that the package only reads (no assignment, no address
  taken, and no use that shares its storage beyond `len`, `cap`, `copy`'s source,
  `append(x, v...)` or a `range` whose items share nothing). A struct type the program cannot
  name is copied without its methods, so a slice of structs prints as Go prints it. `where`
  and `impact` treat such a variable as a constant read from source.

### Changed

- A fact that calls a helper of the setup module attests what the helper uses of the
  surfaces, through the setup module's own helpers at any depth, as a cases table attests
  what its runner uses, in every language: `where` lists the fact under those exports,
  `uncovered` stops listing them, and `impact` lists the fact when one of them changes.
  TypeScript and Python did this already; Go and Rust now do, and in Rust a runner is now
  followed through the setup functions it calls too. In Python, a runner or a named fact
  also attests the private constants it reads (`_PORT`), as a fact that reads them does.
- A fact directive that names a second `@surface`, a second `column=` or `field=`, or an
  empty `column=""` is an error that says so, where the last surface won silently and an
  empty column meant none.

### Fixed

- Directives side by side are one group, and the order they are written in no longer changes
  what any of them binds. A fact followed on its line only by other directives, such as a
  citation, ends its line, its sentence, its paragraph or its table cell as it does when
  written last: `Tax applies in `NL` and: <!-- fact: … --> <!-- test: … -->` above a list
  bound the sentence's `NL`, not the list, and a citation before a fact at the end of a
  sentence or in a cell left the fact with no target. A fact that ends its line also binds
  what starts after the lines of only directives directly below it, as it did when it was
  the last of them.

## [0.2.0] - 2026-11-19

### Added

- A directive may sit alone on the line directly above the sentence it attests, as
  `// @ts-expect-error` sits above its line: a line that holds only directives applies each
  of them to the line below it. A citation there defends the sentence that starts on that
  line, and `where`, `impact`, `audit` and `check --base` read that sentence as its claim; a
  fact there binds what it would bind at the end of that sentence. A directive at the end of
  a line, after text, still applies to what is before it, and one above a list, table or
  fenced block binds it as before. A directive alone on a line, then a blank line, then
  prose, with no prose directly above it, is an error that says to put it directly above the
  sentence or at its end. One alone on a line between two lines of prose now attests the
  line below it, not the paragraph above.
- `<!-- fact field=<path>: … -->` before a fenced JSON, TOML or YAML block compares one value
  in it (`field=ports.0.port`), or a list in it as a set, instead of the whole block. Several
  such facts may sit above one block. The block's value is compared with the fact's as the
  same value, displayed as the project's language displays it (`True` and `None` in Python,
  `<nil>` in Go, `None` in Rust, `1` for `1.0` in TypeScript, Go and Rust), through the adapter protocol's
  new optional capability `display`; with an adapter that does not list it, as JSON writes
  it. The protocol stays at version 1.
- `file("path")` in a fact gives a project file's text, in every language, so a fact can read
  a file that is not a manifest (`USER` in a `Dockerfile`). Like `manifest`, its path is
  written out in the fact, it reaches `impact` when the file changes, and the file must be
  UTF-8. The adapter protocol's `open` gains an optional `texts`.
- A fact reads a constant of its surface from source, exported or not, in every language,
  and a fact that reads only constants runs nothing of the surface. So a surface may be a
  program that nothing imports: a Go `package main`, a Rust binary crate (named by its root
  file, a module's file or the directory of a package with no library), a Python
  `__main__.py`, of each of which a fact reads only constants, or a TypeScript command's
  entry script. A constant is, in TypeScript, a `const` written in literals that the module
  never changes; in Python, a name assigned once at the top level to a literal, an earlier
  constant, or arithmetic and concatenation of these; in Go, any constant, with the value
  the type checker gives it; in Rust, a `const` or `static` of primitive types and
  `Duration`, whose declaration is copied into the fact's program so that it keeps its type,
  and a binary is never built. `where` credits a fact with the constants it reads,
  `uncovered` still lists only exports, and `impact` lists a change to such a constant with
  the facts that read it. Go export data is never read, so a project whose go command is
  newer than the adapter's Go works too.
- The cases of a table-driven test can be cited, in every language, by the name its runner
  prints: TypeScript's `it.each`, `describe.each`, `test.for`, template-literal tables, and
  `for … of` or `.forEach` over a table the file writes; Python's `@pytest.mark.parametrize`
  cases and `with self.subTest(...)` subtests, by the ids pytest and unittest print
  (`"test_total" > "big discount"`); Go's subtests of `t.Run` over a table that is a
  composite literal in the test function; Rust's named cases of rstest (`#[case::name]`)
  and test-case (`#[test_case(… ; "name")]`), as `"tests" > "function" > "name"`, where
  `#[rstest]` and `#[test_case]` functions are tests.
- Each language gives a cases runner the code spans of a cell, as a fact over the cell's
  column reads them: `codeSpans` from `docattest` in TypeScript, `docattest.cells.code_spans`
  in Python, `cells.CodeSpans` in Go, and `docattest_harness::cells::code_spans` in Rust. The
  Go package `github.com/docattest/docattest/adapters/go/cells` imports only the standard
  library, so a module that uses it downloads none of the cores; the Rust helper is in
  `docattest_harness`, the part of every program docattest generates that is the same each
  time, so nothing is added to a project's build. The core and the four helpers are tested
  against one table, `protocol/test-data/code-spans.json`.
- `undocumented` in the config names, by surface, globs of the symbols that are not meant to
  be documented (`{ "default": ["internal*"] }`). `uncovered` leaves them out, the surface
  line counts them apart (`1 not attested by any page, 2 not meant to be documented`), the
  JSON report lists them in the surface's `undocumented`, and a glob that matches no symbol
  is a warning. One a page attests anyway counts as attested.

### Changed

- Rust: a JSON null in a manifest or a snapshot shows as `None`, as a typed `Option::None`
  constant does, where it showed `null`; a null inside an array or object is still written
  as JSON writes it.
- Rust: a fact that names nothing of the project's crates (no export of its surface, no name
  of the setup file, no crate) is built in a program that does not depend on them, so a
  library that does not compile fails only the facts that need it, where it failed every
  fact of its surface. The harness every generated program shares is a package of its own,
  `target/docattest/crate/harness`.
- The coverage line counts proofs and citations apart, and their sum:
  `(41 attestations: 38 proofs, 2 of them weak, and 3 citations)`. The JSON report's
  coverage gains `proofs`. A broken citation now counts as a citation, and errors on a
  snapshot file are no longer counted as attestations.
- `impact` counts the symbols a change affects, not "exported symbols", since they include
  the constants facts read from source (`changes since HEAD affect 2 symbols`), and so does
  the brief of `impact --hook`. The protocol's `affected` answers those constants too.
- A fact over a function, type or variable that a surface declares and does not export
  says so, in every language (`round is a function that surface "default" does not export;
  of its unexported names, a fact reads only constants`).
- A fact that reads only constants no longer loads its surface: in TypeScript it passes
  even when the module throws on import, and in Python its value is the one the source
  assigns even when importing the module changes it. Python's `_PRIVATE` constants, and
  names left out of `__all__`, can now be read.
- The agent skill states fact binding as the core does it: a fact binds the code spans of the
  sentence it ends, which may wrap over lines. It names, for each language, the constants a
  fact reads, the cases a citation can name and the cell helper, and says to name plumbing
  in `undocumented`.
- Every guide says how cases rows run: one after another in one process per table (in
  TypeScript, one worker thread), with tables side by side.
- Go: the adapter loads a project with `golang.org/x/tools/go/packages` and type-checks it
  from source, as gopls does, where it read syntax before. `where`, `uncovered`, `impact` and
  the helpers a cases runner reaches (methods included) follow what names resolve to: a
  local name that shadows an export is not a use of it, a method or field selected on a
  value is a use of the type that declares it, and a method called through an interface is
  the interface's, not its implementations'. A test behind a build constraint names what it
  uses. Installing still needs Go 1.23; `cells` still imports only the standard library.
- Go: `DOCATTEST_GO` names a file called `go`; the adapter puts its directory first on its
  `PATH`, since the package loader runs the first `go` there.
- Go: the guide is split into pages: `adapters/go/docs/examples.md`, `facts.md`, `cases.md`
  and `citations.md`; the README keeps install, configuration, a page, the commands and the
  limits. The Go archive carries the pages beside the README.

### Fixed

- Go and Rust: a block's `runtime.timeoutMs` counts from when the block's own code starts,
  not from when its program is launched, so a quick block no longer times out while the
  system starts the freshly built program (macOS checks a new executable the first time it
  runs, and is slower at it under load). A program that has not started after 10 s is
  killed, and the block fails with `did not start within 10000 ms`. The TypeScript and
  Python adapters already started the clock once a block's worker was ready; their guides
  now say so.

## [0.1.5] - 2026-11-18

### Changed

- `impact --hook` gives the agent a brief instead of the whole report, since it comes after
  every edit: each place that names what the edit affects and that nothing checks, once and
  at most five, then a count of the rest and of the places `docattest check` covers, and
  nothing when no page names what the edit affects. `impact --file` still prints the report.

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

[0.3.0]: https://github.com/docattest/docattest/releases/tag/v0.3.0
[0.2.2]: https://github.com/docattest/docattest/releases/tag/v0.2.2
[0.2.1]: https://github.com/docattest/docattest/releases/tag/v0.2.1
[0.2.0]: https://github.com/docattest/docattest/releases/tag/v0.2.0
[0.1.5]: https://github.com/docattest/docattest/releases/tag/v0.1.5
[0.1.4]: https://github.com/docattest/docattest/releases/tag/v0.1.4
[0.1.3]: https://github.com/docattest/docattest/releases/tag/v0.1.3
[0.1.2]: https://github.com/docattest/docattest/releases/tag/v0.1.2
[0.1.1]: https://github.com/docattest/docattest/releases/tag/v0.1.1
[0.1.0]: https://github.com/docattest/docattest/releases/tag/v0.1.0
