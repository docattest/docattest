# protocol

The contract between the docattest core and a language adapter, and the language-neutral
data that the core and every language package share. Nothing here is written in, or for,
one language.

[PROTOCOL.md](PROTOCOL.md) is the specification: how the core starts an adapter, every
method and rule, and what a launcher does. Read it first when writing an adapter or a
launcher. Around it:

- **The schemas** ([schema/](schema)), JSON Schema (draft 2020-12) of every request,
  response and data type, and of an agent descriptor. The conformance suite, every
  package's tests and `tests/schema.ts` validate against them.
- **Coding-agent descriptors** ([agents/](agents/README.md)), one JSON file per agent, with
  the skill text and the `AGENTS.md` section the core writes. Its README says how to add
  an agent.
- **Shared data**: the core version and protocol number a package is built for, and the
  platforms a core is built for (Rust target, npm `os`/`cpu`, wheel tags, the npm package
  that carries each core). Every launcher, the release scripts and the version check read
  them.

## One source, copied

Each language package ships its own copy of `schema/`, `agents/` (the descriptors and the
agent texts beside them), `core.json` and `platforms.json`, made from this directory when it
is built and never by hand:
`node protocol/sync.ts <dir>` for npm (the build writes it into `dist/`),
`python3 adapters/python/sync_protocol.py` for Python (into
`adapters/python/docattest_python/protocol/`, also run by every build of the wheel), and
`node protocol/sync.ts adapters/go/internal/protocoldata/protocol` for Go, whose copy is
compiled into its two commands. A test in each package fails when its copy differs from
this one.

The core compiles in its own copy too, `node protocol/sync.ts core/protocol`, because a
crate published to crates.io carries only what is inside its directory: `core/build.rs`
embeds every agent descriptor, and `include_str!` embeds `schema/*.json`,
`agents/SKILL.md`, `agents/agents-section.md` and, in a test, `core.json`.

## The conformance suite

PROTOCOL.md §5 defines conformance: an adapter conforms when it passes the conformance
suite. The suite knows no language, and it is what the author of a third-party adapter runs,
often without the rest of this repository, so the core carries it: whoever installed
docattest, from any language package, can run it with nothing else installed.

```sh
docattest conformance run kit.json -- python3 /path/to/adapter
docattest conformance record <log> -- <adapter command...>   # stand between a core and an adapter
docattest conformance validate <log>...                       # hold what was recorded to the schemas
```

Its checks are in `core/src/conformance/`, the schema validator in `core/src/schema/`. The
suites that test *this repository's* implementations against each other (parity, the
benchmark, the mirror test) are in [tests/](../tests/README.md).

## Changing the protocol

1. Change [PROTOCOL.md](PROTOCOL.md) and the schemas together; the schemas are the
   definition of every shape.
2. Change every adapter, the core's client (`core/src/adapter/`), and each adapter's
   conformance kit when the suite needs to know something new about a language.
3. Run every package's tests (they check their copies and run the conformance suite),
   `node tests/schema.ts --candidate core/target/release/docattest`, and the parity
   harnesses ([tests/README.md](../tests/README.md)).

A change that an existing adapter cannot answer is a new protocol version: bump `protocol`
in `core.json` (and the core's own constant), and say so in the changelog.

## How to add a conformance check

1. Add a `Check { name: "<area>: <what must hold>", run: … }` to the `CHECKS` of the file in
   `core/src/conformance/checks/` that groups checks of that area; a new area gets a new
   file, listed in `checks/mod.rs` in the order the checks should run.
2. A check talks to the adapter through `suite.talk` or `suite.session` and states what
   must hold of what comes back (`ensure`, `same`); every line it exchanges is also
   validated against the schemas.
3. If the check needs a piece of code in the adapter's language, add a labelled field to the
   kit's schema (`schema/kit.json`), to `Kit` in `core/src/conformance/kit.rs`, and to every
   adapter's `conformance.json`, and copy the schemas into the packages (above).
4. Run `cargo test` in `core/`, build the core, and run every package's tests: each runs the
   suite against its own adapter.

## How to add a platform

1. Add it to [platforms.json](platforms.json), and copy the data into the packages
   (`node protocol/sync.ts adapters/go/internal/protocoldata/protocol`,
   `node protocol/sync.ts core/protocol` and `python3 adapters/python/sync_protocol.py`).
2. Run `node scripts/version.ts set <the current version>`, which writes the new core
   package into the npm package's `optionalDependencies`.
3. Add the platform to the build matrix and the packaging loops of
   [release.yml](../.github/workflows/release.yml), and to its smoke-test matrix.
4. If its name is not one Go's `GOOS`/`GOARCH` or Python's platform tags already map, teach
   the launchers in `adapters/go/internal/launcher/` and `adapters/python/docattest/`.
