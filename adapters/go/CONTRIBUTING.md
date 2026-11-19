# Working on the Go package

The two commands are thin `main`s under `cmd/`, and `cells` is the one package a user's
code imports: the setup package of a user's pages reads the code spans of a cases table's
cells with it ([cases tables](docs/cases.md)). It
imports only the standard library. <!-- test: endtoend/cells_test.go > "TestCellsImportsOnlyTheStandardLibrary" -->
Everything else is in `internal/`, one package per responsibility, and dependencies point
one way: from the commands, through the session, to the analysis and the runner, and from
there to the protocol's types and the go command. The packages: <!-- fact: Packages() -->
- **`launcher`** finds the core, works out the hand-over (how to start the adapter, what
  command runs docattest) and becomes the core.
- **`bundled`** is the core a launcher carries inside itself: nothing in a checkout; a
  release adds one file per platform that imports that platform's core module.
- **`coremodule`** writes those files and the core modules, and `go run ./tools/coremodule`
  drives it (its top comment says how).
- **`platform`** is the one place that translates Go's names for a platform into
  docattest's and names a program's file on it.
- **`protocol`** holds the protocol's data types and the loop that serves requests.
- **`protocoldata`** is the copy of the repository's `protocol/` data this package ships,
  written by `node protocol/sync.ts adapters/go/internal/protocoldata/protocol` and never by
  hand; it pins the core this package is built for, whose version and protocol number are
  this package's.
- **`language`** answers `hello` and the questions about text, which need no project: how a
  block is read as a Go file, assertions, mentions and expression shapes, doc comments and
  testable examples.
- **`session`** is the project `open` creates. It answers each method from one of the two
  packages that follow.
- **`analysis`** loads the project with `golang.org/x/tools/go/packages` and type-checks it
  from source, and never compiles or runs it: its module and surfaces, `symbols`,
  `affected` and `inCode`, the constants and literal variables facts read from source,
  and the docs helper files of a package; `tests` it reads from syntax. The project at a base revision is the working tree with the changed files put
  back through the loader's overlay, so it is read by the same code.
- **`running`** builds and runs attested code: blocks of pages, testable examples of source
  files, and the programs it generates for facts, cases tables and `observables`. What it
  knows of the project it asks the analysis.
- **`toolchain`** runs the go command with a build overlay, and programs with a time limit.
- **`harness`** is the Go source compiled into generated programs: `program/` is an
  ordinary package, tested here, whose files are embedded as text and written beside a
  generated `main`.
- **`guide`** is the setup of the guide ([README.md](README.md) and the pages of
  [docs/](docs/examples.md)) and this page, which docattest checks with this adapter
  ([docattest.config.json](docattest.config.json)): the runners of the guide's cases
  tables, and what the facts of these pages read by name. The facts read the internal
  packages as their surfaces, and `go.mod` as a manifest.

`conformance.json` is this adapter's kit for the protocol's conformance suite,
`endtoend/` tests the two commands as built, and `testdata/fixture/` is a small Go project
the tests and the smoke test check. `endtoend/` also checks the guide's pages and this
one; the page the guide shows under "A page" is an `md check` block, checked as a page of
the fixture. <!-- test: endtoend/guide_test.go > "TestTheGuideHolds" --> This directory is one of the `members` of the repository's root config, so the
one `docattest check` at the root checks these pages too.

## Tests

```sh
cd adapters/go
go test ./...
```

`internal/analysis` reads `testdata/fixture/` and small projects the tests write;
`internal/running` compiles and runs real code against them with your `go`. `endtoend/`
builds both commands and runs the core's conformance suite with this adapter's kit, and the
real core on the fixture through the launcher (both skipped unless a core is found:
`DOCATTEST_CORE`, <!-- fact @launcher: CoreVariable --> or `core/target/release/docattest`
of this repository). It also builds a launcher with a core inside it, from a stand-in core module, and runs it where there is no other core. <!-- test: endtoend/carried_test.go > "TestALauncherBuiltWithItsCoreNeedsNothingElse" -->
And it holds `cells` to the core: each of a list of cells is a table whose column fact
lists what `CodeSpans` gives for it, so a cell the two read differently fails. <!-- test: endtoend/cells_test.go > "TestCodeSpansAreWhatAColumnFactCompares" -->
`internal/protocoldata` fails when the package's copy of the protocol data differs from
`protocol/` at the root of the repository, <!-- test: internal/protocoldata/protocoldata_test.go > "TestTheCopyIsTheSource" -->
and `internal/platform` when its names disagree with the platforms listed there. <!-- test: internal/platform/platform_test.go > "TestThePlatformsOfTheCore" -->

The suites that test this adapter together with the rest (the mirror test, which runs
benchmark scenarios through the TypeScript, Python, Go and Rust adapters and compares the
verdicts, and the schema recordings) are in [tests/](../../tests/README.md).

## Lint and format

`gofmt` and `go vet`, which come with Go, run in `adapters/go`:

```console check
$ test -z "$(gofmt -l .)" && go vet ./...
```

`testdata/fixture/` is left as written: its pages cite its lines.
