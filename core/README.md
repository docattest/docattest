# docattest core (Rust)

The core is docattest itself, written once for every language: it reads the config, finds
the pages, parses their Markdown, binds each directive to the claim it proves, runs the
proofs, compares what the code says with what the page says, and prints the report and the
exit code. It also answers every other command: the index, `audit`, `check --base`,
`impact`, the hooks and `refresh`. All it needs of a language it asks a language adapter,
which it runs as a child process and speaks to through [PROTOCOL.md](../protocol/PROTOCOL.md).
Every change to docattest's behaviour is made here.

The core's output is held to recordings ([tests/README.md](../tests/README.md), "parity"):
the parity harnesses run it on nearly 500 recorded cases and compare every byte. Comments in
the sources that say a rule is kept "as the recordings hold" mean that changing it changes
recorded output. The npm package ([adapters/typescript](../adapters/typescript/README.md)) is
the TypeScript adapter, the protocol server, the launcher and a programmatic API that runs
this core.

## This page is checked

The core is a Rust library as well as a program, and this page is checked against it by
docattest itself, with the Rust adapter ([adapters/rust](../adapters/rust/README.md)):
`docattest check` here fails when a sentence below with a proof behind it stops being
true. The config is [docattest.config.json](docattest.config.json); each surface is one
module of the `docattest` crate.

The core speaks version `1` of the adapter protocol. <!-- fact @adapter: PROTOCOL -->
A project's config is its `docattest.config.json`. <!-- fact: CONFIG_FILE -->
Unless the config says otherwise, an example may run for `10000` ms. <!-- fact: DEFAULT_TIMEOUT_MS -->
A snapshot is called old after `90` days. <!-- fact: DEFAULT_SNAPSHOT_MAX_AGE_DAYS -->

A coding agent's hook asks the core for one of two things: <!-- fact @agents: ACTIONS -->
- `impact`
- `check`

A config needs its pages, its default surface and its adapter, and gets the rest by
default; one with none of them is refused.
<!-- test: src/config/mod.rs > "tests" > "valid_config_gets_defaults" -->

```rust check
use docattest::config::{parse_config, DEFAULT_TIMEOUT_MS};

let written = r#"{ "pages": ["docs/**/*.md"], "surfaces": { "default": "shop" }, "adapter": "rust" }"#;
let config = parse_config(written).unwrap();
assert_eq!(config.adapter, "rust");
assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
assert!(parse_config("{}").is_err());
```

Wherever the core sorts, it compares strings by UTF-16 code units, as JavaScript does, so
that a report is the same whichever adapter served it. That is not the order of the
characters themselves:

```rust check
use docattest::text::compare_utf16;
use std::cmp::Ordering;

// U+FF5E comes before U+1F600, but its one UTF-16 unit is greater than the first of the
// two that U+1F600 is written with.
assert_eq!("\u{ff5e}".cmp("\u{1f600}"), Ordering::Less);
assert_eq!(compare_utf16("\u{ff5e}", "\u{1f600}"), Ordering::Greater);
```

## How the core is organised

A run starts at the command line and flows one way, from the entry point to the domain and
out to the edges. The `cli` module parses the arguments, finds the project root and hands
the command to one function per command; nothing below it knows there is a command line.
For `check`, the `project` module loads the project: `config` validates the config, `files`
lists the pages and source files on disk (its globs are a port of picomatch, the glob
semantics of every config whichever adapter serves it), and `markdown` turns each page into
the page model, the blocks, tables and code spans that claims are made of. `attestations`
reads the directives on a page, binds each to its claim and plans what must run, and
`project` then runs the check: `compare` judges facts against the values the adapter reads,
the adapter runs examples, citations are judged against the tests it lists, and
`snapshots` supplies recorded values and their age, and `fact_files` the project files facts
read with `manifest("<path>")`, parsed by `manifests`, and with `file("<path>")`, as text,
and what `manifestField` and `matched` take out of them. `report` orders the results, counts
coverage and prints the text or JSON report.

The other commands reuse that pipeline and add one concern each. `changes` is `check
--base`: it plans the project as committed at a ref, in memory and without running
anything, and judges what the change did to every attestation. `impact` finds the pages
that name what a change affects; `audit` finds the lists, tables, blocks and values in
sentences that look copied from a surface and carry no proof; `explain` says what the
planner decided for each directive on a page, beside the results `check` would give it;
`agents` reads the descriptors of coding agents,
which the commands that install hooks and the skill apply. `conformance` stands apart from
the pipeline: it drives an adapter the way `adapter` does, but to hold it to the protocol
(PROTOCOL.md §5), with `schema`, the protocol's JSON Schemas and a validator for them.

At the edges sit the parts that touch the outside world, each behind one module: `adapter`
is the protocol client and the one place the core puts a question to the language, `git`
runs git, `process` starts every child process, and `files` reads the disk. Below
everything are a few small pieces of shared semantics: `text` holds JavaScript's whitespace
and UTF-16 ordering, which docattest's output is defined by, `json` reads and writes JSON in
docattest's key order, `parallel` maps work across threads and `trace` times phases.

## Build and run

```sh
cargo build --release            # target/release/docattest
export DOCATTEST_ADAPTER_TYPESCRIPT="node /path/to/docattest/adapters/typescript/dist/protocol/main.js"
target/release/docattest check --root /path/to/project
```

That is the bare binary, as a developer of the core runs it. **A user never does.** The
core is not installed or run by itself: a language package (npm, PyPI) brings it and
provides the `docattest` command as a launcher, which finds the core, tells it how to
start the package's adapter and what command runs docattest in the project, and becomes it
([PROTOCOL.md](../protocol/PROTOCOL.md) §1, "Launchers"). So the core knows nothing of how
it was installed:

- **The adapter** is the command in `DOCATTEST_ADAPTER_<NAME>` for `"adapter": "<name>"`: a
  JSON array (what a launcher writes; a path may hold spaces), or a command line split on
  spaces. Without the variable, `docattest-adapter-<name>` on the `PATH`. Nowhere else: no
  `node_modules`, no `site-packages`. The one exception is a project's member (`members`,
  [docs/cli.md](../docs/cli.md)) whose entry gives an `adapterCommand`: a launcher can tell
  the core how to start only its own language's adapter, so the config that lists a member
  in another language may say how to start that one. Its words are read from the directory
  of that config, where the adapter is started.
- **The command that runs docattest in the project** (`hooks install`, `agents install`) is
  `DOCATTEST_COMMAND`, <!-- fact @command: COMMAND_VARIABLE --> a shell command to which
  the core appends arguments. `{root}` <!-- fact @command: ROOT --> in it
  is the project root, which the core spells as the reader of the command does: an agent's
  own variable in that agent's hook (its descriptor's `projectRoot`), `.` in `AGENTS.md` and
  in a git hook. Without the variable, `docattest`.
- **The project root**, without `--root`, is the nearest directory at or above the working
  directory that holds a `docattest.config.json`, else the working directory.
  `impact --file` and `impact --hook` look upwards from the edited file first. `--file`
  reads a relative path from the working directory; a path outside the project, or one
  that is neither on disk nor in the diff, is a user error, while the hook stays silent
  about an edit outside the project.
- **Coding agents** are data, as languages are: `../protocol/agents/<name>.json`, one
  descriptor per agent, carried into the binary by `build.rs` (so reading one opens no file
  and starts nothing), with `index.json` naming the default. The `agents` module reads and
  applies them; no source file names an agent, and a test fails when one does
  ([protocol/agents/README.md](../protocol/agents/README.md) says how to add one).
- **`--version --json`** prints `{"version":"0.1.0","protocol":1}` and starts nothing: a
  launcher asks it of a core it did not bring itself. `protocol/core.json` is the same
  pair, pinned by each language package (a test here holds the two together).
- **The clock** is the system's, unless `DOCATTEST_NOW` names another instant (a date,
  `YYYY-MM-DD`, or an RFC 3339 time; anything else is ignored). Only snapshot ages and the
  `fetchedAt` that `refresh` writes depend on it. The programmatic API sets it for
  `env.now`, and the parity harnesses replay their recordings at the instant they were
  made. `DOCATTEST_NETWORK=1` lets `network` blocks run.

Each language package has a launcher. The npm, Python and Go launchers look for the core in
the same three places, in order: `DOCATTEST_CORE`; the binary the package brought; the
per-user cache `<cache>/docattest/core/<version>/<os>-<arch>/docattest`, which every
language's install shares. The Rust package is built with this crate as a library, and so
carries the core in itself. How a package brings the binary, and what remains before a
release, is in "Distribution" below.

The core holds no list of adapters and no language's rules (see "What the core knows of a
language" below). `DOCATTEST_TRACE=1` writes phase and round-trip timings to stderr (for
performance work only; output is otherwise untouched); set to nothing, it is off, in the
core and in every launcher (the Rust one asks the core's `trace::requested`).

The core implements `check [pages…] [--json] [--quiet]` (without pages, also each of the
config's `members`, depth first; the domain is unchanged, `cli/check.rs` walks the members
and `report/members.rs` puts their reports together), `where <symbol> [--json]`,
`uncovered [--surface [name]] [--json]`, `index [--json]`, `--root`, `--help`, the usage
text, `impact [--base <ref>] [--file <path>] [--json]`, `impact --hook` and
`check --hook` (an agent's hook: its event on stdin, one JSON object or nothing on stdout,
both as the agent's descriptor says), `hooks install [--agent <name or file>]`,
`hooks install --git`, `agents install`, `skill install [--agent <name or file>] [--global]
[--force]` and `refresh [name]`. Notes on them:

- `impact` runs git as a child process while the pages are parsed and the adapter answers.
  The `affected` request carries the base contents of every changed path, so they are all
  read: one `git cat-file --batch` for the lot, `git show` for a path that is not a plain
  blob there.
- `impact --hook` is silent unless the edited file is source code of the project, and which
  files are is the adapter's to say: `hello` carries its `sourceFiles` globs and its
  `ignoredNames`. An edit outside the project is answered without an adapter; for any other
  the adapter is started and greeted, cold (no `warm` in `hello`), so it answers without
  loading its compiler. When the file is source code the impact goes on with that same
  adapter.
- Every other command greets with `warm`: the adapter loads its compiler right after it
  answers, while the core lists the project and parses its pages. The listing waits for the
  greeting, because the names it must not walk into come with it.
- `skill install` writes the text of `../protocol/agents/SKILL.md`, embedded when the core
  is built (`include_str!`). Where it goes is the descriptor's `skill`. `agents install` does
  the same with `../protocol/agents/agents-section.md`, the section's text, into the
  project's `AGENTS.md`, which it reads and writes as bytes.
- `hooks install --git` asks git where its hooks are and where the project is below the
  top of the working tree (`git rev-parse --show-prefix --path-format=absolute --git-path
  hooks`), and writes `pre-commit` only when there is none.
- `hooks install` and `refresh` write JSON indented by two spaces, keys in the order the
  file or the source gave them. A file holding a lone surrogate escape is refused rather
  than rewritten.
- `refresh` is the only command that touches the network. Its client is built when a
  snapshot is fetched; no other command runs any of its code.

`audit [--json]`, `inventory [text] [--json]` and `explain <page>[:<line>] [--json]` are
implemented too.
`check --base <ref> [pages…] [--json] [--quiet]` is implemented too: git (a child process)
reads the base on a thread that starts as soon as the adapter has been started. It first
finds the base (the repository, the ref, the files committed there, a valid config among
them), which overlaps with loading the project, and a base that cannot be found fails
before anything runs. Its files are then read while the check runs, planned in memory
without running anything and compared with the working tree. The base is read with the
working tree's adapter, whatever its own config named.

## What the core knows of a language

Nothing. Every question is put to the adapter (PROTOCOL.md §3), and `src/adapter/language.rs`
is the one place the questions are put. The adapter says which files are source code
(`hello.sourceFiles`) and which names are never the project's own (`hello.ignoredNames`,
globs for one name each, and `hello.ignoredMarkers`); the listing of the working tree, the
reading of a base and the agent's hook all use that one answer. It says which words of an
info string are its own (`hello.infoWords`, passed on as an example's `tags`), the
language's words in the core's sentences (`hello.vocabulary`, a suite's `kind`, a package's
`display`), whether tests can be cited (`hello.capabilities`) and whether a value it
observed is text (`Observation.kind`). Asked about code, it answers with the examples in
doc comments (`docComments`) and their prose (`docProse`), whether a `check` block asserts
anything (`assertions`), which symbol a code span names (`mentions`), whether an expression
was only renamed (`expressions`, then `inCode`) and what a configured surface is
(`open.surfaces`, as the config wrote them).

A few things in `src/` still have a language's shape, each on purpose. UTF-16 ordering,
JavaScript's whitespace and picomatch's globs are the semantics of docattest's output and
of the config's globs, the same whichever adapter serves a project; they say nothing about
the language of the project being checked, and PROTOCOL.md §2 states the one an adapter can
observe (order is by UTF-16 code units). `audit` treats `true`, `false`, `null`,
`undefined`, `none`, `yes`, `no`, `on` and `off` as too trivial to be evidence, compared
without case: a word list, not a branch, and `True` and `None` are covered by it. The
config's error for a bad adapter name gives `"typescript"` as its example. No coding
agent's settings, events or JSON are in the core: those are in `../protocol/agents/`, and
what the core keeps is the name `AGENTS.md`, the name of git's `pre-commit` hook, and the
word `matcher` for an entry's filter, which every agent described so far uses. The check
reports a `Diagnostic` with `"compiles"` and `(block line N)`, and a `compile` example with
none, in words that read correctly for a language without a compiler too (Python's
`compile()` is what the Python adapter runs). Finally the unit tests speak TypeScript: they
need a language to answer and run no adapter, so a stand-in compiled for tests only answers
as the TypeScript adapter does.

Git is read in one module, and every child process, git and the adapter alike, is started
by one function that holds a lock meanwhile: a child forked while another thread is between
creating a pipe and marking it close-on-exec would inherit it and keep it open.
`clippy.toml` refuses any other way of starting a child.

## Test

```sh
cargo test                        # unit tests, plus the glob fixtures
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The glob fixtures in `tests/fixtures/` were generated from picomatch 4.0.7, whose semantics
the core keeps, by the script beside them (`gen-glob.ts`). `tests/trace.rs` runs the built
binary to check that a trace nobody reads (stderr closed) changes nothing.

What `cargo test` cannot see is held by the suites in [tests/](../tests/README.md): the
parity harnesses (the core's output on nearly 500 recorded cases), the messages it sends to
every adapter against the protocol's schemas (`tests/schema.ts`, which records real
conversations with `conformance record` and holds them to the schemas with
`conformance validate`), the benchmark and the mirror test.

## Distribution

A plain `npm install docattest` or `pip install docattest` brings the core built for the
machine, as esbuild, Biome and Ruff bring theirs: no install script, no download at first
run.

- **Platforms** are listed once, in `../protocol/platforms.json`: `darwin-arm64`,
  `darwin-x64`, `linux-arm64`, `linux-x64`, `windows-x64`, each with its Rust target, npm
  `os`/`cpu` and wheel tags, and the npm package name of a core
  (`@docattest/core-{platform}`; the launcher, the scripts and the version check all read
  it there, so a change of scope is one line). The npm, Python and Go launchers carry a copy
  (`protocol/sync.ts`, `sync_protocol.py`) to name the supported platforms when there is no core.
- **Linux** is one static build per architecture (`*-unknown-linux-musl`, linked by
  `cargo zigbuild`, which also compiles ring's C for musl): it runs on glibc and musl alike,
  so no package names a `libc` and there is no glibc/musl split to get wrong. **macOS**
  targets 11.0 (arm64) and 10.12 (x64), the versions the wheel tags promise. **Windows** is
  `x86_64-pc-windows-msvc` with the C runtime linked statically.
- **npm**: `node scripts/release/npm-platform.ts --platform <p> --core <binary> --out <dir>
  [--pack <dir>]` makes `@docattest/core-<p>`: `package.json` (exact `os` and `cpu`, the
  release's version, `preferUnplugged`) and `bin/docattest`, executable, nothing else. It
  refuses a binary whose header says it was built for another platform. The docattest
  package lists the five as `optionalDependencies` at its own exact version, so npm
  installs the one that fits. `prepublishOnly` refuses to publish it before all five are on
  the registry (`scripts/release/check-published.ts`).
- **Python**: `python3 adapters/python/build_wheel.py --out <dir> --core <p>=<binary> …`
  builds the package once with setuptools (from a copy; nothing is written into the
  checkout), then one `py3-none-<platform>` wheel per core, with the binary at
  `docattest/bin/docattest` and its executable bit. There is no sdist: it could not build
  the core, so a platform without a wheel fails at `pip install`, and an install from a
  checkout fails at first run with the launcher's message.
- **Go**: `node scripts/release/go-modules.ts --cores <dir>` (in the release's publish
  job) puts each core in a Go module of its own, tags the modules and the Go package that
  requires them in this repository, and checks that `go install` brings a working
  docattest. `node scripts/release/go-archive.ts --platform <p> --core <binary> --out <dir>`
  writes the archive the install tests use: the launcher and the adapter, with the core
  beside them as `docattest-core`.
- **Rust**: three crates, published in this order: `docattest-markdown` (the patched
  markdown-rs below), `docattest-core` (this directory: the library `docattest` and the
  bare core) and `docattest` (`adapters/rust`: the command Rust users install). The core
  compiles in its own copy of the protocol's files, `core/protocol/`, because a crate
  carries only what is inside its directory (`node protocol/sync.ts core/protocol`; a test
  fails when the copy differs).
- **One version**: `node scripts/version.ts check` holds `core/Cargo.toml` (and its lock),
  `protocol/core.json`, the npm package, its five optional dependencies (and its lock),
  the Python package, the Go package's pin and the Rust package to one version (and to the tag,
  `--tag v<version>`);
  `node scripts/version.ts set <version>` writes them all.
- **The release workflow** (`../.github/workflows/release.yml`) builds and tests each core
  on its own platform, makes every package on one runner, installs them fresh on all five
  platforms (`tests/smoke/run.ts`) and publishes only when the owner has enabled it and
  approved the run. [CONTRIBUTING.md](../CONTRIBUTING.md), "Releasing", has the procedure.
- **No core**: the launcher names the platform, the package (or wheel) it expected and the
  fixes: npm's says how optional dependencies get left out (`--omit=optional`, a lockfile
  from another platform) and how to put the package back; on a platform with no core,
  both say to build one and set `DOCATTEST_CORE`. `DOCATTEST_TRACE=1` makes either
  launcher say which core it runs.

What remains:

- Publishing: nothing has been published, and the names (`docattest` on npm and PyPI, the
  `@docattest` scope) are not yet the project's. CONTRIBUTING.md lists what only the owner
  can do.
- The release workflow has not run; Windows and Linux arm64 have not run docattest at all
  (the Linux cores were cross-built and inspected, not run).
- Nothing fills the shared cache. No launcher downloads; a download would need a checksum
  pinned next to the version in `core.json`.

## Changing behaviour

1. State the behaviour in the docs page it belongs to, with its proof, and watch the page
   fail ([CONTRIBUTING.md](../CONTRIBUTING.md), "The docs are the tests").
2. Change the core and its unit tests. Nothing else implements docattest.
3. Run the parity harnesses: the cases whose output changed are listed, each with where it
   differs. When every difference is intended, record them with `--update`
   ([tests/README.md](../tests/README.md), "Recording on purpose"), and review the diff of
   the recordings like any other.
4. `npm run test:docs` runs the docs through the launcher and this core. This repository is
   itself a recorded project, so a change to its docs changes
   `tests/parity/recordings/project/`: record it in the same change.

## How to add a command

A command is a change of behaviour, so it starts as "Changing behaviour" above does: state
it in the docs page it belongs to ([docs/cli.md](../docs/cli.md) for the command line), with
a proof, and watch the page fail. Then:

1. Write the command as one function in its own file in `src/cli/`, named after it (as
   `refresh.rs` holds `refresh`). It is given the words after the command, the project root
   and the parsed options, writes through `Io`, and returns the exit code: `0`, or `1` for a
   failing check. A mistake in the request or the project is an `Error::user`, which the
   command line prints as a message with exit code `1`; any other error is a bug, exit code
   `2`. Keep the work itself in a domain module the function calls, so the command stays a
   thin entry point.
2. Add it to `COMMANDS` in `src/cli/mod.rs`, the table of names that dispatch looks the
   command up in. Dispatch answers `--version` and `--help` itself and finds the root before
   it calls the command.
3. Give a new option one entry in `OPTIONS` in `src/cli/args.rs` and a field on `Options`.
   Options are shared by every command and parsing is strict, so an option the table lacks
   is a usage error.
4. Add its line to the usage text in `src/cli/usage.rs`, in the same columns. The usage text
   is printed word for word in every case that shows it, so the parity harnesses list each
   of those cases as changed.
5. Add unit tests beside the code and parity cases for its output (`tests/parity/extra/`
   for a command on a project, `tests/parity/impact/` for one that reads git or speaks to a
   coding agent), run the harnesses and record the new and changed cases with `--update`
   ([tests/README.md](../tests/README.md), "Recording on purpose").

## Debugging tools

The `examples/` (`dump_page`, `dump_plan`, `check_config`, `tree`) print the core's page
model, plans and config results for any Markdown files, for debugging:

```sh
cargo build --release --examples --manifest-path core/Cargo.toml
core/target/release/examples/dump_page docs/*.md
```

## Dependencies

| crate | why |
|---|---|
| `serde_json` (`preserve_order`) | the protocol and `--json`, keeping docattest's key order |
| `markdown` 1.0.0, vendored in `vendor/markdown` and published as `docattest-markdown` | markdown-rs: the same mdast as `mdast-util-from-markdown`, whose trees the core keeps; three small fixes in `vendor/markdown/PATCHES.md` |
| `yaml-rust2` | frontmatter: whether a page says `docattest: false` |
| `fancy-regex` | executing picomatch's regex sources (lookarounds), and Unicode `\p{P}\p{S}` for autolink literals |
| `chrono` | reading snapshot dates and `DOCATTEST_NOW`, and writing `fetchedAt` |
| `ureq` (`rustls`, `gzip`) | `refresh` only: a small blocking HTTP client, no async runtime; TLS by rustls with the bundled Mozilla roots, so nothing is linked from the system and start-up is unchanged |

No `unsafe`. Malformed projects are user errors with their own messages (the recordings hold them).
