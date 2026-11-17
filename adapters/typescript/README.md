# docattest

**Documentation that CI can fail**, for TypeScript and JavaScript projects.

A Markdown page carries its own proof: examples that run, values in the prose that are
compared with your code, tables whose rows are tests, and citations of tests you already
have. When the code changes and a sentence stops being true, `docattest check` fails at that
page and line. Here the page said a call is tried `3` times and listed two environment
variables; the code now says 5, and reads a third:

```console check cwd=docs/.fixtures/acme-client
$ docattest check
docs/retries.md
  ✗ L1 fact: expected 3, got 5 from RETRY_DEFAULTS.maxAttempts
  ✗ L3 fact: expected 2 items from the page, got 3 from ENV_KEYS; missing: LOG_LEVEL
results: 0 pass, 2 fail, 0 error, 0 skipped
...
FAILED
[exit 1]
```

This is the npm package. The same tool for Python is the PyPI package `docattest`, and
for Go a module that `go install` brings ([adapters/go](../go/README.md)), and for Rust the
crate `docattest` ([adapters/rust](../rust/README.md)); all run one shared core. The project's front page is the
[repository README](../../README.md), and the full documentation is in
[docs/](../../docs/index.md).

## Install

```sh
npm install --save-dev docattest typescript
```

Node 22.12 or later. <!-- fact: manifest("adapters/typescript/package.json").engines.node.replace(">=", "") -->
TypeScript from `5.4` up to (not including) `7` <!-- fact ordered: manifest("adapters/typescript/package.json").peerDependencies.typescript.match(/[\d.]+/g) --> is a
peer dependency: examples are type-checked with your project's own compiler.

The install brings the docattest core, a small native program, as the one optional
dependency of these that is built for your machine: <!-- fact: Object.keys(manifest("adapters/typescript/package.json").optionalDependencies) -->
- `@docattest/core-darwin-arm64`: macOS on Apple silicon
- `@docattest/core-darwin-x64`: macOS on Intel
- `@docattest/core-linux-arm64`: Linux arm64, glibc or musl
- `@docattest/core-linux-x64`: Linux x86-64, glibc or musl
- `@docattest/core-windows-x64`: Windows x64

There is no install script, and nothing is downloaded when docattest runs. If your install
leaves optional dependencies out (`--omit=optional`, or a lockfile written on another
platform), `docattest` says so and how to fix it.

## Quick start

1. Add `docattest.config.json` at the project root:

   ```json
   {
     "pages": ["docs/**/*.md"],
     "surfaces": { "default": "./src/index.ts" },
     "adapter": "typescript"
   }
   ```

   The *surface* is your public API: what pages may import (under your package's name) and
   what facts may read.

2. Add proofs to a page, for example `docs/retries.md`:

   ````md check fixture=docs/.fixtures/acme-client as=docs/retries.md
   A failing call is tried at most `5` times. <!-- fact: RETRY_DEFAULTS.maxAttempts -->

   ```ts check
   import { RETRY_DEFAULTS } from '@acme/client';
   assert.ok(RETRY_DEFAULTS.backoffMs > 0);
   ```
   ````

3. Run `npx docattest check`. It exits 1 when a page and the code disagree.

## What TypeScript brings

| Proof | You write | It fails when |
|---|---|---|
| **Example** | a block tagged `ts check` (runs) or `ts compile` (type-checks only); `js`, `tsx` and friends too | it throws, an `assert` fails, or it no longer type-checks |
| **Fact** | `<!-- fact: expression -->` after a value, or above a list, table column or code block | the value on the page differs from `String(value)` of the expression |
| **Cases** | `<!-- cases: runner -->` above a table, with the runner in your setup module | a row's expected result is not what the runner produces |
| **Citation** | `<!-- test: test/retry.test.ts > "describe title" > "test title" -->` | the file no longer declares an `it` or `test` with that title |
| **Doc comment** | an `@example` whose first line is `// check` or `// compile`, in a file under `sources` | as for an example |

Every example is its own module with a fresh import of your package, runs in a worker with a
time limit, and has the network off: a `check network` block runs only when
`DOCATTEST_NETWORK=1` is set, and is skipped otherwise. Lines above `// ---cut---` run but
are setup that renderers can hide. A surface is an entry file (`./src/index.ts`), a package
name resolved as Node would, or both (`{ "import": "@acme/client", "path":
"packages/client/dist/index.js" }`), which is how a monorepo points a page at one package's
build.

A fact expression reads the surface's exports by their names, `snapshot("name")`, and
`manifest("path")`: the parsed data of a project file such as `package.json`, so a claim
about what the package declares needs no setup code. The versions at the top of this page
are read so ([manifests](../../docs/manifests.md)).

## Commands

Every command (`check`, `where`, `impact` and the rest) is described in the
[repository README](../../README.md#commands) and [the command line](../../docs/cli.md).

## From code

```ts compile
import { createProject } from 'docattest';

const report = await createProject({ root: '.' }).check();
console.log(report.exitCode, report.results.length);
```

`createProject` runs the same core the command runs and returns what it prints with
`--json`, typed. It also takes an in-memory map of files instead of a root.

## What is in this package

The package installs these commands: <!-- fact: Object.keys(manifest("adapters/typescript/package.json").bin) -->
- `docattest`: a launcher that finds the core (`DOCATTEST_CORE`, else the platform package,
  else a cache shared with the other language packages), tells it how to start the
  TypeScript adapter, and becomes it.
- `docattest-adapter-typescript`: the TypeScript adapter, which the core starts and talks to
  over [the adapter protocol](../../protocol/PROTOCOL.md). You never run it yourself.

Its module exports `createProject` and the types it returns: the programmatic API
([from code](#from-code)).

## Working on this package

The sources are grouped by what they serve. The TypeScript language itself (compiling and
running examples, evaluating facts, reading symbols, resolving surfaces, what a change
affects) is under `src/language/`, the only part that loads the compiler. Around it are the
protocol server that answers the core, the `docattest` command and the code that finds the
core it runs, `createProject`, which runs the core for code, and the public types. Tests in
`test/` mirror that layout under `unit/`; `protocol/` drives the built server over its pipes;
and the acceptance and corpus tests run one test per criterion of
[ACCEPTANCE.md](../../docs/design/ACCEPTANCE.md) and per event of
[CORPUS.md](../../docs/design/CORPUS.md) through the built package and the core.
`conformance.json` is this adapter's kit for the protocol's conformance suite.

From the repository root: `npm ci`, `npm run build:core`, `npm run build`, `npm test`.
[CONTRIBUTING.md](../../CONTRIBUTING.md) has the rest.

## License

[MIT](LICENSE)
