# tests

The suites that test docattest as a whole, across the core, the adapters and the packages.
Tests of one language's code stay with it: `npm test` for the npm package
([adapters/typescript/test](../adapters/typescript/test)), `python3 -m unittest` for the
Python package ([adapters/python/tests](../adapters/python/tests)), `go test ./...` for the
Go package ([adapters/go](../adapters/go/CONTRIBUTING.md#tests)), `cargo test` for the Rust
package ([adapters/rust](../adapters/rust/CONTRIBUTING.md#tests)), and `cargo test` for the
core ([core/](../core/README.md)). The conformance suite, which any adapter is held to, is
carried by the core (`docattest conformance run`; [protocol/README.md](../protocol/README.md#the-conformance-suite)).

| suite | proves | run |
|---|---|---|
| [bench/](bench/README.md) | docattest finds drift, and ways of hiding it, in small projects, and stays silent on ordinary ones | `npm run bench` |
| [parity/](parity) | the core prints and writes, byte for byte, what is recorded, on nearly 500 cases | `node tests/parity/<harness>.ts --candidate <core>` |
| [schema.ts](schema.ts) | every line the core really sends to each adapter, and every answer, fits the protocol's schemas | `node tests/schema.ts --candidate <core>` |
| [mirror.ts](mirror.ts) | a TypeScript project and the same project in Python, Go and Rust get the same verdict from the same core | `node tests/mirror.ts --core <core>` |
| [smoke/](smoke) | the packages a release builds install into fresh projects and work, with nothing of this checkout | `node tests/smoke/run.ts --npm <dir> --wheels <dir>` |

Everything here is TypeScript that Node (22.18 or later) runs as it is, with no
dependencies beyond the repository's own `npm ci`; `npm run typecheck` and `npm run lint`
hold it to the same rules as the npm package. What several suites need to set up a project
on disk (its files, a git that reads no user configuration, text written line by line, the
Go and Rust adapters built from the checkout) is in [lib/](lib), one module each; the
repository's root, its release data and reading a command line come from the modules the
scripts share ([scripts/lib/](../scripts/lib)). Build first, from the repository root:

```sh
npm ci
npm run build:core      # core/target/release/docattest
npm run build           # adapters/typescript/dist: the TypeScript adapter and the launcher
```

## bench: the drift benchmark

Each scenario builds a small project, sometimes commits it and applies a change, runs the
npm package's launcher against it (which runs the core built in this checkout) and judges
the JSON it prints. A category counts what was **detected** among its positive scenarios and
its **false alarms** among the negative ones. `left-to-binding` counts drift that `audit`
does not find and only a directive the author writes would catch, so its scenarios are
listed as not met, which is expected. The scenarios that pass are recorded in
[bench/baseline.json](bench/baseline.json), and a run fails when one of them stops passing.
The scenarios and the JSON contract they
judge are described in [bench/README.md](bench/README.md); there is one file of them per
category in [bench/scenarios/](bench/scenarios).

```sh
npm run bench                                                    # prints the table
node scripts/with-core.ts node tests/bench/run.ts --label mine # also writes tests/bench/results/mine.json
```

## parity: the core against its recordings

The core is docattest's only implementation, and what it prints is held to recordings: each
harness runs the core on its cases and compares what it prints
(stdout, stderr, the exit code) and, where a command writes files, what it wrote, with the
recording. Temporary paths, server ports and internal-error stacks are normalised.

| harness | cases | recordings |
|---|---|---|
| [bench.ts](parity/bench.ts) | every benchmark scenario (`run` and `check --json`), and real projects: six commands each, and the median time of three | `recordings/scenarios/`, and `recordings/project/` for this repository |
| [extra.ts](parity/extra.ts) | malformed configs, CRLF, non-ASCII, tables in lists, the queries, the command line | `recordings/extra/` |
| [audit.ts](parity/audit.ts) | `audit` and `inventory`: target kinds, ties, thresholds, the walk's bounds, failing surfaces, searches | `recordings/audit/` |
| [changes.ts](parity/changes.ts) | `check --base`: every change type for every attestation kind, renames, freezes, user errors | `recordings/changes/` |
| [impact.ts](parity/impact.ts) | `impact`, `impact --file`, finding the root, both hook modes, `hooks install` for every shipped agent and for one nobody ships, `hooks install --git`, `agents install`, `skill install`, `refresh` (against a local server) | `recordings/impact/` |

Each harness is a small runner at the path above; its cases live beside it in a directory of
the same name, one file per feature they cover, with the projects they share in a fixtures
module of their own. The runner joins them in a fixed order, which is the order the cases run
and are printed in. Beside the runners are what they all share, one module each: the options
every harness takes, the recordings and how a case is written in one, and the clock cases
run at.

Run them with the bare core, which needs the TypeScript adapter named (do not set a
Python adapter variable; these cases are TypeScript projects):

```sh
core="$PWD/core/target/release/docattest"
export DOCATTEST_ADAPTER_TYPESCRIPT="$(which node) $PWD/adapters/typescript/dist/protocol/main.js"
node tests/parity/bench.ts   --candidate "$core" --project . --runs 1
node tests/parity/extra.ts   --candidate "$core"
node tests/parity/audit.ts   --candidate "$core"
node tests/parity/changes.ts --candidate "$core"
node tests/parity/impact.ts  --candidate "$core"
```

Each prints what differs, with the recording's file and line, and exits 1 when anything
does. `--only <text>` runs the cases whose name contains it. Some details:

- **The clock.** `RECORDED.json` in each recordings directory holds the instant the cases
  were recorded; every case runs with `DOCATTEST_NOW` set to it (and `TZ=UTC`), so snapshot
  ages and `refresh`'s dates come out as recorded on any day.
- **Differing by design** (`extra.ts`): two cases whose behaviour the harness also states
  outright, beyond matching their recordings.
- **Through the npm launcher** (`impact.ts`): every `hooks install` and `agents install`
  case, and one each of `check --hook`, `impact --hook`, `impact`, `impact --file` and
  root-finding, run again through `adapters/typescript/dist/launcher/main.js` told nothing
  but `DOCATTEST_CORE`, and compared with the same recordings.
- **This repository is a recorded project** (`bench.ts --project .`): its docs run
  docattest themselves, so `bench.ts` sets `DOCATTEST_CORE` to the candidate. Because the
  recordings are of this repository's docs, **a change to `docs/`, `README.md` or the docs
  config changes them**: record it in the same change (below). The root config lists the
  core and the Rust, Python and Go guides as `members`, so `check` here checks them too,
  each with its own adapter (which needs cargo, Python and Go), and a change to any of
  their pages changes these recordings as well.
- **Projects outside the repository** keep their recordings outside it:
  `--project <dir> --recordings <dir>`, as many pairs as you like.
- **Timing.** Each project's recordings hold `timing.json`, the medians of `check --json`,
  `index --json` and `audit --json` when it was recorded. Every run prints its own medians
  beside them; `--max-ratio <x>` fails a run slower than `x` times the recorded median, and
  `--record-timing <label>` stores a run's medians under a label.

### Recording on purpose

When a change to the core is meant to change what it prints:

1. Run the harnesses: every case whose output changed is listed, with where.
2. When every difference is intended, run the same command with `--update` and
   `--source "<why>"` (which each written file says it was recorded from). Only cases that
   differ or are new are written; each is printed (`recorded (changed): …`), recordings of
   cases that no longer exist are removed, and nothing is written silently.
3. Review the diff of the recordings like any other code, and commit it with the change.

The format of a recording, one text file per case named after it, is described at the top
of [parity/recording-format.ts](parity/recording-format.ts).

## schema.ts: the messages against the schemas

Runs the core on real projects (this repository with the TypeScript adapter, the Python
fixture with the Python adapter, the Go and Rust fixtures with their adapters, which it
builds when there is a `go` command and a `cargo`) with a recorder between the core and the adapter
(`docattest conformance record`), and holds every recorded line, both ways, to
`protocol/schema/` (`docattest conformance validate`). It prints
`schemas: N recorded lines, 0 that do not fit`.

## mirror.ts: the same verdict in every language

Takes 18 benchmark scenarios written for TypeScript, and 9 of its own
([mirror/scenarios.ts](mirror/scenarios.ts): facts over unexported constants and functions
and over a program's constants, `file()`, `field=`, citations of a table-driven test's
rows, `undocumented`, and a cases runner that reads its cells with the language's cell
helper), translates each page and source file to Python, Go and Rust by hand (keeping line
numbers, so the scenario's own expectations still hold; any other file is the same in
every language), runs every translation through the same core, and requires the same
verdict and exit code as the TypeScript side. A language's translation is one file of
tables in [mirror/](mirror), beside the shape of a translation that they share. A Go export
begins with a capital, so the Go side's `Retry` and `Parse` are restated as the scenario
names them before its expectation judges what the core printed. Every project can import
this checkout's docattest package, as one that installed it can: the TypeScript side links
it into `node_modules`, the Python side has it on `PYTHONPATH`, and the Go module requires
it with a `replace` (the Rust side's helper comes with the crate docattest generates). It
needs the built TypeScript adapter and `python3`, and builds the Go adapter when there is a
`go` command and the Rust adapter when there is a `cargo` (a side whose tool is missing is
left out, and it says so); `--keep` keeps the projects it made.

## smoke: a fresh install

Installs the packages a release builds into new projects, as a user would, with a new home
directory, npm cache and pip cache, and no `DOCATTEST_*` variable: `docattest --version`,
which core the launcher runs, `check` passing and failing, `hooks install`, the programmatic
API and its types; the Python package with no Node on the `PATH`; both packages in one
project, each checking the other's language; npm without the platform package (the launcher
says what is missing); and pip refusing a wheel for another platform, when one is given.
With `--go <dir>` it also unpacks the Go package's archive for this platform and checks the
Go fixture with it, which needs a `go` command, as a Go project does.

The release workflow runs it on all five platforms; CI runs it for Linux x64. To run it here
(macOS arm64 in this example; build the other wheel's core with
`cargo build --release --target x86_64-apple-darwin` if you want the refusal step):

```sh
npm run build:core && npm run build
out="$(mktemp -d)"
node scripts/release/npm-platform.ts --platform darwin-arm64 \
  --core core/target/release/docattest --out "$out/npm/core-darwin-arm64" --pack "$out/npm"
npm pack --workspace adapters/typescript --pack-destination "$out/npm"
python3 adapters/python/build_wheel.py --out "$out/wheels" \
  --core darwin-arm64=core/target/release/docattest
node scripts/release/go-archive.ts --platform darwin-arm64 \
  --core core/target/release/docattest --out "$out/go"
node tests/smoke/run.ts --npm "$out/npm" --wheels "$out/wheels" --go "$out/go"
```

It ends `smoke: all N steps passed on <platform>`.
