# docattest

[![docs: attested](https://img.shields.io/badge/docs-attested-17a35e)](https://docattest.github.io/docattest/)
[![GitHub Marketplace](https://img.shields.io/badge/GitHub_Marketplace-docattest-2088FF?logo=githubactions&logoColor=white)](https://github.com/marketplace/actions/docattest)

**Documentation that CI can fail.**

Docs go stale because nothing checks them. docattest lets a Markdown page carry its own
proof: examples that run, values in the prose that are compared with your code, tables
whose rows are tests. When the code changes and a sentence stops being true, the build
fails at that page and line.

- **Works on the docs you already have.** Proofs are HTML comments and tagged code
  blocks, invisible when the page is rendered.
- **TypeScript/JavaScript, Python, Go and Rust** today, from one shared core.
- **No generated files, no lockfiles, no model calls.** The page runs.
- **Adopt one page at a time.** A page with no proofs is reported, not failed.

Try it in your browser: the [playground](https://docattest.github.io/docattest/playground.html)
lets you break a small project's code and watch docattest catch its docs. The
[website](https://docattest.github.io/docattest/) has the short tour.

> **Status:** `0.2.2` <!-- fact: manifest("adapters/typescript/package.json").version -->, pre-release: on npm, PyPI and
> crates.io, and for Go by `go install` (see [Install](#install)).

## What it looks like

A page says what the code does, and a comment says where the truth lives:

```console check cwd=docs/.fixtures/acme-client
$ cat docs/retries.md
A failing call is tried at most `3` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->

The client reads these environment variables: <!-- fact: ENV_KEYS -->
- `API_TOKEN`
- `REGION`
```

Someone raises the retry limit to 5 and adds `LOG_LEVEL`, and forgets the docs:

```console check cwd=docs/.fixtures/acme-client
$ docattest check
docs/retries.md
  ✗ L1 fact: expected 3, got 5 from RETRY_DEFAULTS.maxAttempts
  ✗ L3 fact: missing: LOG_LEVEL (2 items on the page, 3 in ENV_KEYS)
results: 0 pass, 2 fail, 0 error, 0 skipped
...
FAILED
[exit 1]
```

## Contents

- [Install](#install)
- [Quick start](#quick-start)
- [Kinds of proof](#kinds-of-proof)
- [Commands](#commands)
- [How to](#how-to)
- [Configuration](#configuration)
- [Documentation](#documentation)
- [FAQ](#faq)
- [Repository layout](#repository-layout)
- [Contributing](#contributing)

## Install

**TypeScript or JavaScript**

```sh
npm install --save-dev docattest typescript
```

With pnpm, add it to the workspace's root with `pnpm add -D -w docattest typescript` and run
it with `pnpm exec docattest check`.

Node 22.12 or later. <!-- fact: manifest("adapters/typescript/package.json").engines.node.replace(">=", "") -->
That is the Node that runs docattest, in CI too, even when your package supports older ones.
TypeScript from `5.4` up to (not including) `7` <!-- fact ordered: manifest("adapters/typescript/package.json").peerDependencies.typescript.match(/[\d.]+/g) --> is a
peer dependency, so examples are type-checked with your project's own compiler.

**Python**

```sh
pip install docattest
```

Python 3.10 or later. <!-- fact: manifest("adapters/python/pyproject.toml").project["requires-python"].replace(">=", "") -->
No dependencies, and Node is not needed.

**Go**

```sh
go install github.com/docattest/docattest/adapters/go/cmd/...@latest
```

Installing needs Go 1.23 or later. <!-- fact: manifest("adapters/go/go.mod")["go"] -->
After that, your project's own `go` command (1.21 or later) is all it needs (on recent
macOS, 1.23 or later, or `CGO_ENABLED=0`); Node and Python are not. The
[Go guide](adapters/go/README.md) has the details.

**Rust**

```sh
cargo install docattest
```

Your project's own `cargo` is all it needs. The core is written in Rust, so this builds
it into the command. The [Rust guide](adapters/rust/README.md) has the details.

Every package brings the docattest core, a small native program, built for your machine:
`darwin-arm64` or `darwin-x64` (macOS), `linux-arm64` or `linux-x64` (Linux, glibc and
musl alike), or `windows-x64` <!-- fact: Object.keys(manifest("protocol/platforms.json").platforms) -->.
There is no install script and nothing is downloaded when docattest runs.

## Quick start

### TypeScript

1. Add `docattest.config.json` at the project root:

   ```json
   {
     "pages": ["docs/**/*.md"],
     "surfaces": { "default": "./src/index.ts" },
     "adapter": "typescript"
   }
   ```

   The *surface* is your public API: what pages may import (under your package name) and
   what facts may read.

2. Add proofs to a page, for example `docs/retries.md`:

   ````md check fixture=docs/.fixtures/acme-client as=docs/retries.md
   # Retries

   A failing call is tried at most `5` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->

   Backoff is never zero.

   ```ts check
   import { RETRY_DEFAULTS } from '@acme/client';
   assert.ok(RETRY_DEFAULTS.backoffMs > 0);
   ```
   ````

3. Run it:

   ```sh
   npx docattest check
   ```

   In a monorepo, build the workspace packages the surface imports first: an import of one
   resolves to what its `package.json` exports, usually its built `dist`, as it does for
   your users.

The [TypeScript guide](adapters/typescript/README.md) covers facts that read constants from
source, a command's entry script included, citations of table-driven tests (`it.each`,
`for … of`), and `codeSpans` for cases runners.

### Python

1. Add `docattest.config.json` at the project root:

   ```json
   {
     "pages": ["README.md", "docs/**/*.md"],
     "surfaces": { "default": "shoplib" },
     "adapter": "python"
   }
   ```

2. Add proofs to a page:

   ````md check fixture=adapters/python/tests/fixture
   Prices are in `EUR` unless told otherwise. <!-- fact: DEFAULT_CURRENCY -->

   ```pycon check
   >>> from shoplib import Money, total
   >>> total([Money(1000)], "NL")
   Money(cents=1210, currency='EUR')
   ```
   ````

3. Run it:

   ```sh
   docattest check
   ```

The [Python guide](adapters/python/README.md) covers docstring examples, sessions that
continue across blocks, facts that read constants from source (a `__main__.py`'s too),
citations of `parametrize` cases and subtests, `docattest.cells.code_spans` for cases
runners, and which interpreter runs your code.

### Go

1. Add `docattest.config.json` at the project root:

   ```json
   {
     "pages": ["README.md", "docs/**/*.md"],
     "surfaces": { "default": "." },
     "adapter": "go"
   }
   ```

   The surface is a package of your module: `"."` is the one at the project root.

2. Add proofs to a page:

   ````md check fixture=adapters/go/testdata/fixture
   Prices are in `EUR` unless told otherwise. <!-- fact: DefaultCurrency -->

   ```go check
   import (
   	"fmt"

   	"example.com/shop"
   )

   fmt.Println(shop.EUR(750))
   // Output: 7.50 EUR
   ```
   ````

3. Run it:

   ```sh
   docattest check
   ```

The [Go guide](adapters/go/README.md) has a page for each kind of proof:
[examples](adapters/go/docs/examples.md) (statements, declarations or a whole program, and
testable `Example` functions), [facts](adapters/go/docs/facts.md) (constants and literal
variables read from source, a `package main`'s too, and docs helper files that spare a
package exports made only for its pages), [cases](adapters/go/docs/cases.md) (the setup package and
`cells.CodeSpans`) and [citations](adapters/go/docs/citations.md) (the subtests of a
table-driven test).

### Rust

1. Add `docattest.config.json` at the project root:

   ```json
   {
     "pages": ["README.md", "docs/**/*.md"],
     "sources": ["src/**/*.rs"],
     "surfaces": { "default": "shop" },
     "adapter": "rust"
   }
   ```

   The surface is a module of your crate: `"shop"` is its root, `"shop::shipping"` a module
   below it.

2. Add proofs to a page:

   ````md check fixture=adapters/rust/tests/fixture
   Prices are in `EUR` unless told otherwise. <!-- fact: DEFAULT_CURRENCY -->

   ```rust check
   use shop::Money;

   assert_eq!(Money::eur(750).to_string(), "7.50 EUR");
   ```
   ````

3. Run it:

   ```sh
   docattest check
   ```

The [Rust guide](adapters/rust/README.md) covers doc-comment examples, facts that read
constants from source (a binary's too), `#[cfg(docattest)]` docs helpers, citations of
rstest and test-case cases, the setup file and `docattest_harness::cells::code_spans`, and
where blocks are built.

The rest of this README writes commands as `npx docattest …`; in a Python, Go or Rust
project drop the `npx`.

## Kinds of proof

<!-- fact column=Reported: KINDS -->
| Proof | Reported | You write | It fails when |
|---|---|---|---|
| **Example** | `example` | a code block tagged `check` (runs) or `compile` (type-checks only); a `console check` block of commands and their output, or an `md check fixture=<dir>` sample page, which docattest runs itself | the code throws, an assertion fails, or it no longer compiles; a command prints or exits otherwise; a proof on the sample page fails |
| **Fact** | `fact` | `<!-- fact: expression -->` after a value, or above a list, table column, code block or one field of a JSON, TOML or YAML block | the page's value differs from what the expression gives |
| **Cases** | `cases` | `<!-- cases: runner -->` above a table | a row's expected result is not what the runner produces |
| **Citation** | `test` | `<!-- test: file > "test name" -->` after a sentence | the test it names no longer exists |
| **Snapshot** | `snapshot` | a fact that reads `snapshot("name")` | the pinned data from an outside system disagrees with the page, or is too old |

A fact reads your code through the surface, and a project file such as `package.json`,
`pyproject.toml` or `go.mod` through `manifest("path")`, so a claim like "Node 22.12 or
later" is held to what the package declares. Any other file, such as a `Dockerfile`, it
reads as text through `file("path")` ([manifests](docs/manifests.md)).

Examples in doc comments (`@example` in TypeScript, `>>>` in Python docstrings, fenced
blocks in Rust) and Go's testable `Example` functions are checked too, when the config
lists your source files under `sources`. *Reported* is the kind a result carries in the
report.

[PATTERN.md](docs/design/PATTERN.md) explains how to choose a proof for a sentence.

## Commands

<!-- fact column=Command: COMMANDS -->
| Command | Usage | What it does |
|---|---|---|
| `check` | `check [pages…]` | Run every proof, or just those pages. Exit 1 on a failure. |
| `check` | `check --base <ref>` | Also fail when a proof was removed since `<ref>` but its claim was kept. |
| `check` | `check --report <format>=<path>` | Also write the report for another CI service: JUnit XML (`junit`) or GitLab Code Quality (`codequality`). |
| `where` | `where <symbol>` | The pages, lines and sentences that describe a symbol. |
| `uncovered` | `uncovered` | Public symbols no page describes yet. |
| `index` | `index` | The whole symbol-to-pages map. |
| `audit` | `audit` | Lists, tables, blocks and values in sentences that look copied from the code and have no proof, with the directive to add. |
| `inventory` | `inventory [text]` | Values a fact could read, each with its expression, the setup module's named facts included. |
| `impact` | `impact [--base <ref>]` | The documented symbols your diff affects and the prose that names them. |
| `impact` | `impact --file <path>` | The same for one edited file's change; silent when nothing is affected. |
| `refresh` | `refresh [name]` | Refetch snapshots from their source. |
| `hooks` | `hooks install` | Add hooks to a coding agent's settings, or `--git` for a pre-commit hook. |
| `agents` | `agents install` | Write a docattest section into `AGENTS.md`. |
| `skill` | `skill install` | Install the authoring guide where your coding agent looks for skills. |
| `conformance` | `conformance run <kit.json> -- <adapter command>` | Hold a language adapter to the protocol; for adapter authors. |

Most commands take `--json`. `check --quiet` prints only what needs attention, then `ok` or
`FAILED`: what a coding agent or a git hook should read. `--root <dir>` sets the project; by
default it is the nearest directory at or above the current one that has a
`docattest.config.json`.

**Exit codes:** 0 everything holds, 1 a proof failed or the project is misconfigured,
2 a bug in docattest.

## How to

### Add docattest to existing docs

You do not need to rewrite anything. Start with one page:

```sh
npx docattest audit                 # suggests directives for lists, tables and values copied from the code
npx docattest inventory maxAttempts # finds the expression for a value you see in the prose
npx docattest check docs/retries.md # check just the page you are working on
npx docattest uncovered             # what is left
```

Pages without proofs are counted as uncovered and never fail the build.

### Run it in CI

Run it after your build and before any docs-site build.

```yaml
name: docs
on: [push, pull_request]
jobs:
  docattest:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0 # only needed for --base
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
      - run: npm ci
      - run: npm run build --if-present
      - uses: docattest/docattest@v0.2.2
        with:
          args: --base origin/main
```

The [GitHub Action](action/README.md), also
[on the GitHub Marketplace](https://github.com/marketplace/actions/docattest), runs the
docattest your project installed, marks each claim that fails on its line of the pull
request, and lists them on the run's summary page.
For a Python project, replace the Node steps with `actions/setup-python` and
`pip install docattest`. For a Go project, replace them with `actions/setup-go` and the
`go install` above; for a Rust project, with a Rust toolchain and `cargo install docattest`.
On another CI service, run `docattest check --base origin/main` in their place, and add
`--report` to give it a file it shows: JUnit XML, which most services read as test results,
or a GitLab Code Quality report ([the command line](docs/cli.md#reports-for-other-ci-services)).

### Run it in GitLab CI

```yaml
docattest:
  image: node:22
  variables:
    GIT_DEPTH: 0 # the whole history, so --base finds the merge request's base commit
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
  script:
    - npm ci
    - npm run build --if-present
    # --base only in a merge request pipeline, where GitLab names the commit it diffs against
    - npx docattest check ${CI_MERGE_REQUEST_DIFF_BASE_SHA:+--base "$CI_MERGE_REQUEST_DIFF_BASE_SHA"}
      --report junit=docattest.xml --report codequality=gl-code-quality.json
  artifacts:
    when: always # the reports matter most when the check fails
    reports:
      junit: docattest.xml
      codequality: gl-code-quality.json
```

A merge request then shows each claim that fails with its page and line, among its test
results and its code quality findings. The base commit is an ancestor of the merge
request's branch, so the full clone holds it and nothing else is fetched. For Python, Go or
Rust, use that language's image and install docattest as [Install](#install) says.

### Show that your docs are checked

Put the badge at the top of your README, so a reader knows its claims are tested:

```markdown
[![docs: attested](https://img.shields.io/badge/docs-attested-17a35e)](https://docattest.github.io/docattest/)
```

### Stop proofs being quietly removed

A failing proof can be "fixed" by deleting it. `--base` compares with a git ref and fails
when a claim is still on the page but its proof is gone:

```sh
npx docattest check --base origin/main
```

It also warns when a page was frozen, a `check` block was downgraded to `compile`, or an
expression was weakened. See [changes](docs/changes.md).

### See which docs a code change affects

Before you change a symbol, or after:

```sh
npx docattest where RETRY_DEFAULTS  # every sentence that describes it
npx docattest impact                # the prose your uncommitted changes touch
npx docattest impact --base main    # the prose a branch touches
```

### Check before every commit

```sh
npx docattest hooks install --git
```

This writes a `pre-commit` hook that runs `docattest check --quiet`. An existing hook is
never modified; docattest prints the line to add to it instead.

### Use it with a coding agent

docattest asks two things of any agent: run `impact --file <path>` after editing code, and
`check` before finishing.

```sh
npx docattest agents install        # tells every agent that reads AGENTS.md
npx docattest hooks install         # Claude Code runs both automatically
npx docattest skill install         # teaches the agent how to write proofs
```

`--agent gemini`, `--agent cursor` or `--agent ./my-agent.json` targets another agent.
See [agents](docs/agents.md).

### Document something outside your repo

A statement about another system cannot be proven, only pinned. Save its data as a
snapshot, let facts read it with `snapshot("regions")`, and refresh it on a schedule:

```sh
npx docattest refresh
```

`check` warns when a snapshot is older than `maxAgeDays`. See [snapshots](docs/snapshots.md).

### Document what a manifest declares

"Node 22.12 or later" and "Python 3.10+" are true because a manifest says so, not the code.
A fact reads the manifest, and reads any other file, such as a `Dockerfile`, as text:

```md check fixture=docs/.fixtures/acme-client
Node 22.12 or later. <!-- fact: manifest("package.json").engines.node.replace(">=", "") -->

The image runs as user `10001`. <!-- fact: file("Dockerfile").match(/^USER (\d+)$/m)[1] -->
```

JSON, TOML, YAML and `go.mod` are read as data, with `manifest("path")`; `file("path")`
reads any file as its text. The path is relative to the project root. See
[manifests](docs/manifests.md).

### Keep a page out of the check

List it under `frozen` in the config. A frozen page is listed in the report and never
read; use it for changelogs, design notes and old release pages.

### Use it from code

```ts compile
import { createProject } from 'docattest';

const report = await createProject({ root: '.' }).check();
console.log(report.exitCode, report.results.length);
```

`createProject` also takes an in-memory map of files, which is how docattest's own docs
test it.

## Configuration

`docattest.config.json`, at the project root:

<!-- fact column=Key: CONFIG_KEYS -->
| Key | Meaning |
|---|---|
| `pages` | Globs of the Markdown pages to check. Required. What git ignores is never a page, and a glob cannot start with `!`: use `frozen` to leave pages out. |
| `adapter` | The project's language: `"typescript"`, `"python"`, `"go"` or `"rust"`. Required. |
| `surfaces` | Your public API, by name. `default` is required. An entry is an entry file, a package or module name (a package's directory or import path in Go, a module path in Rust), or `{ "import": "<name>", "path": "<file>" }`. |
| `undocumented` | By surface, globs of the symbols that are not meant to be documented, written as `uncovered` prints their names: `{ "default": ["internal*", "Program*"] }`. `uncovered` leaves them out, the report counts them apart, and a glob that matches no symbol is a warning. See [duality](docs/duality.md#symbols-not-meant-to-be-documented-are-counted-apart). |
| `sources` | Globs of source files whose doc-comment examples are checked. |
| `frozen` | Globs of pages that are listed in the report but never checked. |
| `setup` | A module that provides `cases` runners and named `facts`. |
| `snapshots` | `{ "dir": "docs/.snapshots", "maxAgeDays": 30 }`. A snapshot is old after `90` <!-- fact: DEFAULT_SNAPSHOT_MAX_AGE_DAYS --> days unless `maxAgeDays` says otherwise. |
| `runtime` | `{ "timeoutMs": 5000, "workers": 4 }`. The defaults are `10000` <!-- fact: DEFAULT_TIMEOUT_MS --> ms per block and one worker per CPU. |
| `ignoreTags` | Words a docs renderer adds to code fences (such as `twoslash`), accepted and otherwise ignored. |
| `members` | Directories below this one that hold projects of their own, in any language, which `check` checks with this one. An entry is a directory, or `{ "path": "<dir>", "adapterCommand": ["<program>", …] }` when the launcher you run cannot start that project's adapter. See [the command line](docs/cli.md). |
| `commands` | The programs your console blocks run, each with globs of the files it is built from: `{ "mytool": ["src/**/*.ts"] }`. `impact` then lists the blocks that run one whose files changed. See [impact](docs/impact.md). |
| `$schema` | Ignored, so an editor can be pointed at a schema. |

Examples run in isolated workers with the network off; set `DOCATTEST_NETWORK=1` to allow it.

## Documentation

These pages are docattest's own documentation, checked by docattest on every build of this
repository; all but [why](docs/why.md), which is reasoning, carry proofs.

| Page | What it covers |
|------|----------------|
| [examples](docs/examples.md) | `check` and `compile` blocks, isolation, network, timeouts |
| [commands and sample pages](docs/console.md) | `console check` transcripts and `md check` sample pages, which docattest runs itself |
| [facts](docs/facts.md) | binding a value, list, column or block to an expression |
| [cases](docs/cases.md) | tables run row by row |
| [citations](docs/citations.md) | binding a sentence to an existing test |
| [snapshots](docs/snapshots.md) | pinning facts about outside systems, and `refresh` |
| [manifests](docs/manifests.md) | facts over project files: `package.json`, `pyproject.toml`, `go.mod`, and any file as text |
| [pages](docs/pages.md) | living and frozen pages, doc comments, checking some pages |
| [surfaces](docs/surfaces.md) | what pages import and where it resolves |
| [duality](docs/duality.md) | `where`, `uncovered` and the symbol index |
| [audit](docs/audit.md) | finding lists, tables and values nobody has attested yet |
| [inventory](docs/inventory.md) | the values a fact could read, and the expressions that read them |
| [changes](docs/changes.md) | `check --base`: removed, frozen, downgraded and weakened proofs |
| [impact](docs/impact.md) | from a code diff to the prose it touches |
| [agents](docs/agents.md) | coding agents, `AGENTS.md`, the git hook, agent descriptors |
| [report](docs/report.md) | results, coverage, exit codes, the text report |
| [command line](docs/cli.md) | the commands in detail |
| [adapters](docs/adapters.md) | the language boundary |
| [why](docs/why.md) | the reasoning behind the design |

The language packages have their own pages: [TypeScript and
JavaScript](adapters/typescript/README.md), [Python](adapters/python/README.md),
[Go](adapters/go/README.md) and [Rust](adapters/rust/README.md).

Going deeper: [PATTERN.md](docs/design/PATTERN.md) (how to write an attested page),
[SPEC.md](docs/design/SPEC.md) (the machinery), [PROTOCOL.md](protocol/PROTOCOL.md) (how
the core talks to a language adapter), [ACCEPTANCE.md](docs/design/ACCEPTANCE.md) and
[CORPUS.md](docs/design/CORPUS.md) (the criteria and drift events it is tested
against).

## FAQ

**Does it change how my docs render?**
No. Directives are HTML comments, and `check` is an extra word on a code fence that
renderers ignore.

**How is this different from doctest or snippet testing?**
Those check that code blocks run. docattest also checks the prose: values in sentences,
lists, table columns and whole blocks, and it maps every public symbol back to the sentences that
describe it.

**Do I have to cover everything?**
No. Uncovered pages and symbols are reported, never failed. `check --base` only guards
what is already proven.

**Is it safe to run on pull requests from forks?**
It runs the code in your docs and your package, like a test suite does. Treat it as you
treat your tests. See [SECURITY.md](SECURITY.md).

**Can I use it for another language?**
The core is language-neutral and talks to a per-language adapter over a small protocol.
[adapters/README.md](adapters/README.md) is the guide to adding a language,
[PROTOCOL.md](protocol/PROTOCOL.md) is the protocol, and
`docattest conformance run` checks an adapter.

**reStructuredText?**
Not yet. Pages are Markdown.

## Repository layout

[CONTRIBUTING.md](CONTRIBUTING.md#where-things-live) maps the repository: the core, one
package per language, the protocol, the docs and the test suites.

## Contributing

Issues and pull requests are welcome. To build from source you need
Node 22.18+ <!-- fact: manifest("package.json").engines.node.replace(">=", "") -->, Rust
(stable) and, for the Python and Go packages,
Python 3.10+ <!-- fact: manifest("adapters/python/pyproject.toml").project["requires-python"].replace(">=", "") -->
and Go 1.23+ <!-- fact: manifest("adapters/go/go.mod").go -->.

```sh
npm ci
npm run build:core     # the core: cargo build --release in core/
npm run build          # the TypeScript adapter, launcher and API
npm test               # adapter, protocol, launcher and API tests
npm run test:docs      # docattest run on its own docs
npm run typecheck && npm run lint && npm run format:check
```

All behaviour lives in the Rust core in [core/](core/README.md); the packages are thin
launchers plus a language adapter. The docs are the acceptance tests: a behaviour change
starts as an attested section in `docs/` that fails until the core does what it says.
[CONTRIBUTING.md](CONTRIBUTING.md) has the details, including how to cut a release; please
read the [code of conduct](CODE_OF_CONDUCT.md), and report security issues as
[SECURITY.md](SECURITY.md) says, not in public issues.

## License

[MIT](LICENSE)
