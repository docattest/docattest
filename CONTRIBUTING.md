# Contributing

Thanks for helping. docattest is small on purpose; the bar for a change is that a
maintainer can read it once and merge it. Everyone taking part is expected to follow the
[code of conduct](CODE_OF_CONDUCT.md). Report a security problem as [SECURITY.md](SECURITY.md)
says, never in a public issue.

## Set up

You need Node 22.18 or later (the repository's tools are TypeScript, which Node runs as it
is from 22.18; users of the npm package need 22.12), a stable Rust toolchain with `clippy`
and `rustfmt`, Python 3.10 or later, Go 1.23 or later, and git.

```sh
npm ci                  # the repository's tools, and the npm package as a workspace
npm run build:core      # core/target/release/docattest
npm run build           # adapters/typescript/dist
npm test                # the npm package's tests (they build dist first, and run the core)
npm run test:docs       # docattest checks its own docs
```

For the Python package's linter, make a virtualenv of its own:

```sh
cd adapters/python
python3 -m venv .venv && .venv/bin/pip install --group lint    # ruff and pyright, pinned in pyproject.toml
```

## Where things live

| path | what |
|---|---|
| [core/](core/README.md) | **all behaviour**: configuration, pages, directives, binding, comparison, the index, the report, `audit`, `check --base`, `impact`, hooks, exit codes |
| [protocol/](protocol/README.md) | the contract between the core and a language adapter ([PROTOCOL.md](protocol/PROTOCOL.md)), its schemas, the conformance suite, coding-agent descriptors, and the data every package copies |
| [adapters/typescript/](adapters/typescript/README.md) | the npm package: the TypeScript adapter (`src/language/`) and its protocol server (`src/protocol/`), the launcher (`src/launcher/`, with `src/core-binary/`, which finds the core), `createProject` (`src/client/`) and the public types (`src/api/`) |
| [adapters/python/](adapters/python/README.md) | the PyPI package: the Python adapter (`docattest_python/`) and launcher (`docattest/`) |
| [adapters/rust/](adapters/rust/README.md) | the Rust package, one crate: the Rust adapter (`src/bin/adapter.rs`) and the `docattest` command built with the core inside it (`src/bin/docattest.rs`) |
| [adapters/go/](adapters/go/README.md) | the Go package, one Go module: the Go adapter (`cmd/docattest-adapter-go`) and launcher (`cmd/docattest`), both built from `internal/` |
| [docs/](docs/README.md) | the documentation, which is also the acceptance test suite |
| [tests/](tests/README.md) | the suites that test the whole: benchmark, parity, schemas, mirror, install smoke test |
| [scripts/](scripts/README.md) | the version check, packaging, the link check, `with-core.ts` |
| [.github/](.github) | CI and release workflows, issue and pull request templates |
| `docattest.config.json`, `package.json` | the config that checks this repository's own docs; repository-wide scripts and tools (the npm package is the workspace `adapters/typescript`) |

**Every change to what docattest does is made in the core.** A package is a launcher plus
a language adapter; an adapter answers only what needs its language
([adapters/README.md](adapters/README.md) explains the split, and how to add a language).
A coding agent is a descriptor, not code ([protocol/agents/README.md](protocol/agents/README.md)).

In this checkout the npm package's launcher and API find the core built in
`core/target/release/docattest` because the npm scripts run them through
`scripts/with-core.ts`, which sets `DOCATTEST_CORE`. To run docattest by hand, do the same:

```sh
node scripts/with-core.ts node adapters/typescript/dist/launcher/main.js check
```

## Changing behaviour: the docs are the tests

Every page in `docs/` is an attested page: its examples run, its facts are checked against
the package, its tables run row by row. `npm run test:docs` must pass, with `docs/why.md`
the only uncovered page; the README and the design documents in `docs/design/` are frozen.

A behaviour change goes docs first:

1. Write the section that states the new behaviour, and its proof, in the page it belongs
   to.
2. Run `npm run build:core && node scripts/with-core.ts node
   adapters/typescript/dist/launcher/main.js check docs/<page>.md` and watch it fail for the
   right reason.
3. Implement it in the core, with unit tests, until it passes. Then run everything,
   including the parity harnesses, and record the cases whose output changed on purpose
   (below).

Never make a failing page pass by weakening an assertion, deleting an attestation or
freezing the page. If a change alters behaviour an existing assertion pins, change that
assertion deliberately and say why in the pull request. Update
[SPEC.md](docs/design/SPEC.md) for any behaviour you add or change, and
[PATTERN.md](docs/design/PATTERN.md) when authoring is affected.

`adapters/typescript/test/acceptance.test.ts` and `corpus.test.ts` hold one test to each
criterion in [ACCEPTANCE.md](docs/design/ACCEPTANCE.md) and each event in
[CORPUS.md](docs/design/CORPUS.md); they run through the built package and the core.

A change to the protocol changes [PROTOCOL.md](protocol/PROTOCOL.md), the schemas, every
adapter and the core's client together ([protocol/README.md](protocol/README.md)).

## Recording on purpose

The core's output is pinned by recordings in `tests/parity/recordings/`. A change that
alters what it prints shows up in the parity harnesses; when every difference is intended,
run the same harness again with `--update --source "<why>"`, which writes only the cases
that changed and lists each one, and commit the recordings with the change.
[tests/README.md](tests/README.md) has the commands.

This repository is one of the recorded projects (`tests/parity/recordings/project/`), so a
change to `docs/`, `README.md` or `docattest.config.json` changes its recordings too: record
them in the same change with `node tests/parity/bench.ts --candidate <core> --project .
--runs 1 --update --source "<why>"`.

## Checks

Everything CI runs, from the repository root:

| command | what it checks |
|---|---|
| `npm run versions` | every package carries one version (`scripts/version.ts`) |
| `npm run build:core`, `npm run build` | the core, and the npm package |
| `npm run typecheck` | the npm package's sources, tests and the docs' setup module, and the repository's tools (`tsconfig.json`: `tests/`, `scripts/`, `protocol/`, the core's fixture generators) |
| `npm run lint` | ESLint over all TypeScript |
| `npm run format:check` | Prettier over all TypeScript, JavaScript, JSON and YAML |
| `npm run links` | every relative link in every Markdown file |
| `npm test` | the npm package's unit, protocol, acceptance and corpus tests |
| `npm run test:docs` | docattest on its own docs, and on the projects the root config lists as `members`: the core's README and doc comments, and the Rust, Python and Go guides, each with its own adapter (so it needs cargo, Python and Go) |
| `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | the core (in `core/`) |
| `python3 -m unittest discover -s tests` | the Python package (in `adapters/python/`; set `DOCATTEST_CORE` to the built core) |
| `.venv/bin/ruff check .`, `.venv/bin/ruff format --check .`, `.venv/bin/pyright` | the Python package's lint, format and types (in `adapters/python/`) |
| `go test ./...` | the Go package (in `adapters/go/`; set `DOCATTEST_CORE` to the built core, or build it in `core/target/release`) |
| `test -z "$(gofmt -l .)"`, `go vet ./...` | the Go package's format and lint (in `adapters/go/`) |
| `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` | the Rust package (in `adapters/rust/`) |
| `node tests/parity/*.ts`, `tests/schema.ts`, `tests/mirror.ts` | the core against its recordings, the protocol messages against the schemas, the adapters against each other ([tests/README.md](tests/README.md)) |
| `npm run bench` | the drift benchmark |

## Coding standards

The same rules everywhere. Each language's formatter and linter enforce style and
correctness, and CI fails when one is broken; the size limits below are for reviewers.

- **Small and obvious.** A function does one job in at most 60 lines, with at most 5
  parameters and a complexity of 10 (15 in Go, where every error check is a branch); no
  file is over 400 lines, generated data and `core/vendor/` aside. Functions in test code
  are not held to the function limits. Reviewers apply these numbers; no linter does, so
  the linters stay about correctness and style. Comments say why, not what. No dead code,
  no commented-out code, no `TODO`: open an issue instead.
- **Tools are TypeScript**, designed like the package: typed data, small typed libraries
  and thin entry points, run by Node as they are (type stripping: imports name `.ts` files,
  and only erasable syntax). No JavaScript file of our own but `eslint.config.js`, which
  ESLint can load as TypeScript only through another dependency (it is type-checked with
  `// @ts-check`).
- **User-facing messages** name the page and line. A user error is the user's to fix and
  starts `docattest: `; an internal error is a bug in docattest (exit 2).
- **No behaviour without a proof**: a docs page, a unit test, or a recording.

| language | formatter | linter | rules worth knowing |
|---|---|---|---|
| Rust (`core/`) | `cargo fmt` (`rustfmt.toml`: width 110) | `cargo clippy --all-targets -- -D warnings` (`clippy.toml` refuses any way of starting a child process but `process::spawn`) | no `unsafe` |
| TypeScript (`adapters/typescript/`) | Prettier (`.prettierrc.json`: width 100, single quotes) | ESLint `strictTypeChecked` and `stylisticTypeChecked` (`eslint.config.js`) | strict TypeScript with `noUncheckedIndexedAccess` and `exactOptionalPropertyTypes`; every export has a one-line doc comment; no `any` (untyped values come in as `unknown` and are narrowed); lines under 100; only `src/language` loads the compiler (lint enforces it), so the launcher and the API start in milliseconds |
| Python (`adapters/python/`) | `ruff format` (width 110) | `ruff check` (pycodestyle, pyflakes, isort, bugbear, pyupgrade, annotations; `pyproject.toml`) and `pyright` (strict for the package) | standard library only; the worker (`workers/worker.py` and its parts in `workers/docattest_worker/`) may run on the project's own interpreter |
| Rust (`adapters/rust/`) | `cargo fmt` (the core's `rustfmt.toml`) | `cargo clippy --all-targets -- -D warnings` | no `unsafe` but two calls that have no safe form (signalling a process group, setting a variable before any thread starts), each with its reason; `src/harness/` uses only `std`, because it is compiled into programs generated in other people's projects |
| Go (`adapters/go/`) | `gofmt` | `go vet ./...` | standard library only; every package under `internal/`; the source compiled into generated programs (`internal/harness/program/`) declares only names that begin `docattest`, so none can meet a name of a project's |
| TypeScript tools (`tests/`, `scripts/`, `protocol/`, `core/tests/fixtures/gen-*`) | Prettier | the same ESLint rules, type-checked by the root `tsconfig.json` with the package's flags | Node 22.18 or later runs them as they are, no dependencies; test code (`tests/`, `core/tests/`) is tables of cases and is exempt from the line length limit |

Markdown is never reformatted by a tool: pages are attested by line, and tables are aligned
by hand. Keep lines under 100 characters where you can.

## Pull requests

- One change per pull request, with the docs, tests and recordings that go with it.
- Run the checks above; say in the description what you ran and anything you could not.
- Add a line to `CHANGELOG.md` under the next version for anything a user would notice.

## Releasing

A release is one version everywhere: the core (`core/Cargo.toml`), its pin
(`protocol/core.json`), the npm package `docattest` (`adapters/typescript/package.json`)
and the five npm packages that carry the core (`@docattest/core-<os>-<arch>`, its
`optionalDependencies`), the Python distribution `docattest`
(`adapters/python/pyproject.toml`), the Go package, whose version is the core's it
pins (`adapters/go/internal/protocoldata/protocol/core.json`), and the Rust package
(`adapters/rust/Cargo.toml`). Everything is built by
[release.yml](.github/workflows/release.yml) from a tag, and, after the first release (see
the owner's list below), published by it too.

1. Bump: `node scripts/version.ts set 0.2.0` writes the version into every place (the
   Cargo and npm lockfiles too); `npm run versions` (also run by CI and the release
   workflow) fails when any two disagree. Move the `CHANGELOG.md` entries under the new
   version, and run every check.
2. Tag the commit `v0.2.0` and push the tag. The workflow checks the tag against every
   version, builds and tests the core on each platform's own runner
   (`aarch64-apple-darwin` on macOS arm64, `x86_64-apple-darwin` on macOS Intel, static
   musl builds for Linux on x64 and arm64 runners, `x86_64-pc-windows-msvc` with a static C
   runtime), makes the five npm platform packages (`scripts/release/npm-platform.ts`),
   the docattest npm package (`npm pack --workspace adapters/typescript`), the five
   wheels (`adapters/python/build_wheel.py`) and the five Go archives
   (`scripts/release/go-archive.ts`) on one runner, and installs them into fresh
   projects on all five platforms ([tests/smoke/](tests/README.md#smoke-a-fresh-install)).
   Every package is kept as an artifact of the run (`npm-packages`, `wheels`,
   `go-archives`); the Go archives are used by those install tests only.
   `workflow_dispatch` runs the same without a tag, to try a release before tagging one.
3. Publish: only when the repository variable `DOCATTEST_PUBLISH` is `true`, and only once
   someone approves the run in the `release` environment. The job publishes the five
   platform packages first, waits until the registry lists them, then `docattest` (npm,
   with provenance), the wheels (PyPI), the three crates in the order they need each
   other (`docattest-markdown`, `docattest-core`, `docattest`), and the Go package
   (`scripts/release/go-modules.ts`): each platform's core as a Go module, committed
   outside `main` and tagged `go-core/core-<platform>/v0.2.0`, the Go package requiring
   them, committed on the release's commit and tagged `adapters/go/v0.2.0`, then a real
   `go install` of it, then the earlier releases' core tags deleted (Go's module proxy
   keeps every version it served), so a clone carries one release's cores. npm, PyPI and
   crates.io authenticate by OIDC (trusted publishing); the Go tags are pushed with the
   workflow's own token. Every step leaves alone what an earlier run already published,
   so a stopped run can be run again.

To try the packages on your own machine, see
[tests/README.md](tests/README.md#smoke-a-fresh-install).

The repository is https://github.com/docattest/docattest, and every package points to it:
the npm package's `repository` (npm provenance refuses a package without one that matches
the repository the workflow runs in; the platform packages copy it), the wheel's project
links and its README, whose relative links `adapters/python/setup.py` makes absolute for
PyPI's page, and the Go package's module path. What only the owner can do, once, before
the first release, in this order:

1. **npm**: create the `docattest` organization (scope `@docattest`) and make sure the
   unscoped name `docattest` is free or yours. If the scope cannot be had, change
   `npmPackage` in `protocol/platforms.json` (and the five names in
   `adapters/typescript/package.json`'s `optionalDependencies`, then
   `npm install --package-lock-only`); the launcher, the scripts and the version check
   read the name there.
2. **Security reports**: turn on private vulnerability reporting in the repository's
   settings, which [SECURITY.md](SECURITY.md) points to.
3. **PyPI**: register a pending trusted publisher for the project `docattest` (owner, this
   repository, workflow `release.yml`, environment `release`); PyPI creates the project on
   the first upload.
4. **npm trusted publishing**: on each of the six packages' settings page, add GitHub
   Actions as trusted publisher: this repository, workflow `release.yml`, environment
   `release`, with **Allow npm publish** ticked (without it npm holds every version until
   a maintainer approves it). A new trusted publisher expires unless a release publishes
   through it within two days. npm adds one only to a package that exists, so a package
   new to npm is first published with a short-lived token, set as `NODE_AUTH_TOKEN` on the
   publish step for that one release.
5. **crates.io**: on each of the three crates' settings page, add a trusted publisher:
   this repository, workflow `release.yml`, environment `release`. crates.io adds one
   only to a crate that exists, so a crate new to crates.io is first published with an
   API token (scope `publish-new` and `publish-update`) saved as the repository secret
   `CARGO_REGISTRY_TOKEN`: the publish job uses it when it is set. Delete the secret, and
   revoke the token, once every crate has its trusted publisher.
6. **GitHub**: create the environment `release` with required reviewers (and, if wanted,
   limited to tags `v*`), then set the repository variable `DOCATTEST_PUBLISH` to `true`.
   Until it is set, the publish job is skipped.
