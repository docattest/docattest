# docattest for Go

[docattest](../../README.md) for Go projects: documentation that CI can fail. Your Markdown
pages carry examples that run, facts that are compared with values in your code, tables
whose rows are tests, and citations of tests you already have.

Try it in your browser first: the [playground](https://docattest.github.io/docattest/playground.html)
lets you break a small project's code and watch docattest catch its docs.

This package is what a Go user installs. It provides two commands:

- **`docattest`**, the command you run. It is a launcher: it finds the docattest core (one
  binary for every language, which reads the pages and writes the report), tells it how to
  start the Go adapter and what command runs docattest in your project, and becomes it.
- **`docattest-adapter-go`**, the adapter: the program that answers everything that needs
  Go. You do not run it yourself. It speaks [the adapter protocol](../../protocol/PROTOCOL.md),
  version `1`. <!-- fact: Hello().Protocol -->

Both are Go programs. Beyond the standard library they import one module,
`golang.org/x/tools`, <!-- fact: regexp.MustCompile(`(?m)^require (\S+) `).FindStringSubmatch(file("go.mod"))[1] -->
whose `go/packages` lists your project's packages, as gopls does; `go/types` checks them. Your project needs a
`go` command (1.21 or later) on the `PATH`, as it does to be built: packages are listed
and examples and facts compiled by it. Node and Python are never needed.

On recent macOS, use Go 1.23 or later, or set `CGO_ENABLED=0`. Every program docattest
builds imports `net` (to keep examples off the network), and an earlier Go links such a
program without the `LC_UUID` load command, which the macOS loader now refuses to run.

## Install

```sh
go install github.com/docattest/docattest/adapters/go/cmd/...@latest
```

It needs Go 1.23 or later. <!-- fact: manifest("go.mod")["go"] -->
`go install` puts the commands in `$(go env GOBIN)`, or in `$(go env GOPATH)/bin` when
`GOBIN` is not set, and that directory must be on your `PATH` for a shell to find
`docattest`:

```sh
export PATH="$(go env GOPATH)/bin:$PATH"
```

That one `go install` brings the launcher (`docattest`), the Go adapter
(`docattest-adapter-go`) and the core for your platform:

<!-- fact column=platform @protocoldata: Platforms() -->
| system | platform |
|---|---|
| macOS on Apple silicon | `darwin-arm64` |
| macOS on Intel | `darwin-x64` |
| Linux x86-64, glibc or musl | `linux-x64` |
| Linux arm64, glibc or musl | `linux-arm64` |
| Windows x64 | `windows-x64` |

The go command builds Go source and cannot bring a program that is not Go, and the core is
written in Rust. So a release carries each platform's core in a small Go module of its
own, as bytes (`github.com/docattest/docattest/go-core/core-<platform>`), and the launcher
imports the one for the platform it is built for: the go command downloads that module like
any dependency, checked against `go.sum` and Go's checksum database, and the launcher writes
the core to the shared cache (3, below) the first time it runs. Nothing is downloaded when
docattest runs. A `docattest` built from a checkout carries no core, and says so on its
first run, with where it looked.

The launcher looks for a core in this order, the same as the npm and PyPI packages do:
<!-- test: internal/launcher/launcher_test.go > "TestFindCoreInOrder" -->

1. `DOCATTEST_CORE`, the path of a core binary. <!-- fact @launcher: CoreVariable -->
   It is asked for its version (`--version --json`) and refused when it speaks another
   protocol than this package. <!-- test: internal/launcher/launcher_test.go > "TestACoreSomeonePointsAtIsAsked" -->
2. `docattest-core` beside the `docattest` command (`docattest-core.exe` on Windows):
   where the release's Go archives, which its install tests use, carry it.
3. `<cache>/core/<version>/<os>-<arch>/docattest` (`docattest.exe` on Windows), a cache
   shared with the npm and PyPI packages. The version is the one this package pins
   (`internal/protocoldata/protocol/core.json`).

`<cache>` is `DOCATTEST_CACHE` when it is set, else a directory of the system's own (`~` is
the home directory; `XDG_CACHE_HOME` counts only when it is an absolute path):

<!-- cases: cache -->
| system  | environment          | cache                        |
|---------|----------------------|------------------------------|
| any     | DOCATTEST_CACHE=/c   | /c                           |
| macOS   |                      | ~/Library/Caches/docattest   |
| Linux   | XDG_CACHE_HOME=/x    | /x/docattest                 |
| Linux   | XDG_CACHE_HOME=x     | ~/.cache/docattest           |
| Linux   |                      | ~/.cache/docattest           |
| Windows | LOCALAPPDATA=/l      | /l/docattest                 |
| Windows |                      | ~/AppData/Local/docattest    |

The launcher says which core it runs when the core's trace is asked for, by setting
`DOCATTEST_TRACE` to anything but nothing: <!-- fact @launcher: TraceVariable -->
`[trace launcher] core <path>`, the path of the core it found: the shared cache's, once
the launcher has written its core there. The npm, PyPI and Rust packages install a command named `docattest` too, and a shell
runs the first one on the `PATH`. The Rust one has the core built into it: when the trace
says `core (built in)`, that is the `docattest` that ran, and it writes nothing to the
cache.

From a checkout of the repository, without installing anything:

```sh
cargo build --release --manifest-path core/Cargo.toml
export DOCATTEST_CORE="$PWD/core/target/release/docattest"
(cd adapters/go && go build -o "$PWD/bin/" ./cmd/...)
adapters/go/bin/docattest check --root adapters/go/testdata/fixture
```

`docattest hooks install`, `hooks install --git` and `agents install` write down a command
that runs docattest: `docattest` when that is what a shell finds on the `PATH`, else this
command by its full path. To have them write something else, set `DOCATTEST_COMMAND`. <!-- fact @launcher: CommandVariable -->

A bare core, run without the launcher, finds the adapter as `docattest-adapter-go` on the
`PATH`, or through a variable, `DOCATTEST_ADAPTER_GO`: <!-- fact @launcher: AdapterVariable -->
a JSON array (`["/path/to/docattest-adapter-go"]`) or a command line split on spaces.
`DOCATTEST_GO` names the `go` command the adapter runs, when it is not the one on the
`PATH`. It is a file called `go`: the adapter puts its directory first on its own `PATH`,
since the package loader runs the first `go` there. <!-- test: internal/toolchain/toolchain_test.go > "TestTheChosenGoIsFirstOnThePath" -->
When that command cannot be run, every proof that needs it says why, as on this page of
the package's fixture:

```console check cwd=testdata/fixture
$ DOCATTEST_GO=/no/such/go docattest check --quiet docs/shipping.md
docs/shipping.md
  ! L3 fact: surface "shipping" failed to load: the go command could not be run: fork/exec /no/such/go: no such file or directory
...
[exit 1]
```

## Configure

`docattest.config.json` at the project root:

```json
{
  "pages": ["README.md", "docs/**/*.md"],
  "sources": ["**/*.go"],
  "surfaces": { "default": ".", "shipping": "./shipping" },
  "setup": "docs/docattest_setup.go",
  "adapter": "go"
}
```

- **`surfaces`** name what pages are allowed to observe. A surface is one Go package, a
  `package main` included (its constants are what [facts](docs/facts.md) can read). An entry is
  - a path, `"."` or `"./shipping"` (or a file of the package, `"./shipping/quote.go"`):
    the package in that directory of the project, imported under its import path in the
    module;
  - an import path, `"example.com/shop/shipping"`: a package of the module, or of one of
    its dependencies, which the go command is asked to find;
  - both, `{ "import": "example.com/shop", "path": "internal/shop" }`: examples import
    `import`, and the package is read from `path`.
- **What a surface exports** is every exported name its package declares at the top
  level: functions, types, variables and constants. Methods and fields belong to their
  type. `uncovered` lists the exports no page attests.
- **What an attestation uses** (what `where` and `uncovered` count) is read as `go/types`
  resolves its names. A local name that shadows an export is not a use of it, and a method
  or field selected on a value is a use of the type that declares it, which for one
  promoted from an embedded type is that type. A method called through an interface is a
  use of the interface, not of the types that implement it: which method runs is known only
  when the program runs, and the code that made the value names its type where it made it. <!-- test: internal/analysis/symbols_test.go > "TestSymbolsAreResolvedByType" -->
- **The module** is the nearest `go.mod` at or above the first surface that names a path,
  else at or above the project root. One project is one module.
- **`sources`** are the Go files whose testable examples are checked
  ([examples](docs/examples.md#testable-examples-in-source-files)).
- **`setup`** is a package of the module, named by its directory or one of its files, with
  the runners of cases tables and what facts may use by name.

## A page

````markdown check fixture=testdata/fixture
# Pricing

A discount is a whole percentage taken off a price.

```go check
import (
	"fmt"

	"example.com/shop"
)

price, err := shop.ApplyDiscount(shop.EUR(1000), 25)
if err != nil {
	panic(err)
}
fmt.Println(price)
// Output: 7.50 EUR
```

Prices are in `EUR` unless told otherwise. <!-- fact: DefaultCurrency -->

Tax is known for these regions: <!-- fact: TaxRates -->
- `NL`
- `DE`
- `FR`

<!-- cases: discount -->
| case           | price | percent | result |
|----------------|-------|---------|--------|
| a quarter off  | 1000  | 25      | 750    |
| rounds half up | 999   | 50      | 500    |

Discounts above the maximum are refused. <!-- test: shop_test.go > "TestTotals" > "rejects big discounts" -->
````

```sh
docattest check          # exit 1 when a page and the code disagree
docattest uncovered      # exports no page attests
docattest where Total    # the pages, lines and sentences that attest a symbol
docattest audit          # lists, tables and values that look copied from the code, with no directive
docattest impact         # after an edit: the exports it affects and the prose that names them
```

## When a page and the code disagree

`check` names each proof that fails, with its line and why. This package's fixture,
`testdata/fixture`, has a page on which every proof fails in its own way: a block that
prints something other than what it states, one that does not compile, one that panics
and one that runs out of time, each error at its line in the block; facts whose values
differ, and one over a function the surface does not export; rows of cases tables, one of
them failed by a runner that panics, after which the next row still runs; citations of a
test that is not there and of a benchmark, which name the tests the file has. A network
block, on another page, is skipped:

```console check cwd=testdata/fixture
$ docattest check --quiet docs/broken.md
docs/broken.md
  ✗ L5 example: expected output "7.00 EUR", got "7.50 EUR"
  ✗ L17 example: undefined: shop.AplyDiscount (block line 3)
  ✗ L23 example: panic: no carrier serves "XX" (block line 4)
  ✗ L31 example: timed out after 3000 ms
...
  ✗ L48 fact: expected 60, got 50 from MaxDiscount
  ✗ L50 fact: missing: DE, FR; unexpected: BE (2 items on the page, 3 in TaxRates)
  ! L54 fact: fact `round(50)`: round is a function that surface "default" does not export; of its unexported names, a fact reads only constants
  ✗ L59 cases: docs/broken.md:59 wrong on purpose: expected 800, got 750
  ✗ L60 cases: docs/broken.md:60 too much: The discount is outside the allowed range
  ! L62 cases: cases runner "nope" at L62 is not in Cases of docs/docattest_setup.go (known: delivery, discount, total)
  ! L67 test: shop_test.go declares no test "TestThatWasRenamed"; tests there: "ExampleApplyDiscount", "ExampleTotal", "TestTotals", "TestTotals" > "adds tax", "TestTotals" > "rejects big discounts"
  ! L69 test: "BenchmarkTotal" in shop_test.go is a benchmark, not a test; tests there: "ExampleApplyDiscount", "ExampleTotal", "TestTotals", "TestTotals" > "adds tax", "TestTotals" > "rejects big discounts"
  ✗ L74 cases: docs/broken.md:74 a price in words: panic: strconv.Atoi: parsing "ten": invalid syntax
  ✗ L75 cases: docs/broken.md:75 the next row still runs: expected 1200, got 1210
...
[exit 1]
$ docattest check docs/shipping.md
docs/shipping.md
...
  - L21 example: network example skipped: the network is off (set DOCATTEST_NETWORK=1 to run it)
...
```

## What each attestation means in Go

Each kind of attestation has a page of its own.

### Examples

[Examples](docs/examples.md): what a block tagged `go` is, how it is built and run, and
the testable examples of source files.

### Facts

[Facts](docs/facts.md): what a fact expression may use, how its value is compared with the
page, and the constants it reads from source, unexported or in a `package main`.

### Cases tables

[Cases tables and the setup package](docs/cases.md): the runners of cases tables, the
`cells` package that reads their cells, and how their rows run.

### Citations

[Citations](docs/citations.md): what a test is in a Go test file, and the subtests of a
table-driven test that can be cited.

## What is never your project's own

The adapter tells the core to leave files and directories named `vendor`, `testdata` and
`node_modules` alone, wherever they are. <!-- fact: Hello().IgnoredNames --> Hidden
directories are left out by the core itself.

## Limits

- One module per project: the generated programs are compiled in it.
- A method called through an interface counts for the interface, in the index and in
  `impact`, and not for what implements it: a change to an implementation affects the
  exports that make a value of its type, not those that only call through the interface. <!-- test: internal/analysis/affected_test.go > "TestAffectedFollowsWhatNamesResolveTo" -->
- `impact` cuts a package into its top-level names. The constants of one `const (…)`
  declaration are one unit, since `iota` ties them together.
- A fact has one value ([facts](docs/facts.md)), and a block cannot mix top-level function
  declarations with loose statements: write a whole file, or use function literals.
- Windows has no process groups here: a block that overruns is killed, and a program it
  started may live on. This path has not been run on Windows.

## Working on this package

[CONTRIBUTING.md](CONTRIBUTING.md) maps the package's sources, says how its tests run, and
how the guide is checked.

## License

[MIT](LICENSE)
