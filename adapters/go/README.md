# docattest for Go

[docattest](../../README.md) for Go projects: documentation that CI can fail. Your Markdown
pages carry examples that run, facts that are compared with values in your code, tables
whose rows are tests, and citations of tests you already have.

This package is what a Go user installs. It provides two commands:

- **`docattest`**, the command you run. It is a launcher: it finds the docattest core (one
  binary for every language, which reads the pages and writes the report), tells it how to
  start the Go adapter and what command runs docattest in your project, and becomes it.
- **`docattest-adapter-go`**, the adapter: the program that answers everything that needs
  Go. You do not run it yourself. It speaks [the adapter protocol](../../protocol/PROTOCOL.md),
  version `1`. <!-- fact: Hello().Protocol -->

Both are Go programs with no dependencies beyond the standard library. Your project needs
a `go` command (1.21 or later) on the `PATH`, as it does to be built: examples and facts
are compiled by it. Node and Python are never needed.

On recent macOS, use Go 1.23 or later, or set `CGO_ENABLED=0`. Every program docattest
builds imports `net` (to keep examples off the network), and an earlier Go links such a
program without the `LC_UUID` load command, which the macOS loader now refuses to run.

## Install

Unpack the release archive for your platform, `docattest-go-<version>-<platform>.tar.gz`,
and put its directory on your `PATH` (or call `docattest` by its path):

<!-- fact column=archive @protocoldata: Platforms() -->
| platform | archive |
|---|---|
| macOS on Apple silicon | `darwin-arm64` |
| macOS on Intel | `darwin-x64` |
| Linux x86-64, glibc or musl | `linux-x64` |
| Linux arm64, glibc or musl | `linux-arm64` |
| Windows x64 | `windows-x64` |

The archive holds three programs side by side: `docattest`, `docattest-adapter-go` and
`docattest-core`, the core. Nothing is downloaded when docattest runs.

`go install` is meant to do the same in one line. It needs Go 1.23 or later. <!-- fact: manifest("go.mod")["go"] -->

```sh
go install github.com/docattest/docattest/adapters/go/cmd/...@latest
```

The go command builds Go source and cannot bring a program that is not Go, and the core is
written in Rust. So a release carries each platform's core in a small Go module of its
own, as bytes, and the launcher imports the one for the platform it is built for: the go
command downloads that module like any dependency, checked against `go.sum`, and the
launcher writes the core to the shared cache (3, below) the first time it runs. Nothing
is downloaded when docattest runs. **This is built and tested, and not yet published**:
where the core modules are hosted is still to be decided
([CONTRIBUTING.md](../../CONTRIBUTING.md), "Releasing"), and until it is, a `docattest`
built by `go install` or from a checkout carries no core, and says so on its first run,
with where it looked.

The launcher looks for a core in this order, the same as the npm and PyPI packages do:
<!-- test: internal/launcher/launcher_test.go > "TestFindCoreInOrder" -->

1. `DOCATTEST_CORE`, the path of a core binary. <!-- fact @launcher: CoreVariable -->
   It is asked for its version (`--version --json`) and refused when it speaks another
   protocol than this package. <!-- test: internal/launcher/launcher_test.go > "TestACoreSomeonePointsAtIsAsked" -->
2. `docattest-core` beside the `docattest` command (`docattest-core.exe` on Windows):
   where a release archive carries it.
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
`DOCATTEST_TRACE` to anything but nothing. <!-- fact @launcher: TraceVariable -->

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
`PATH`. When that command cannot be run, every proof that needs it says why, as on this page
of the package's fixture:

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

- **`surfaces`** name what pages are allowed to observe. A surface is one Go package. An
  entry is
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
- **The module** is the nearest `go.mod` at or above the first surface that names a path,
  else at or above the project root. One project is one module.
- **`sources`** are the Go files whose testable examples are checked (see below).
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
differ, and one over a name the surface does not export; rows of cases tables, one of
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
  ✗ L50 fact: expected 2 items from the page, got 3 from TaxRates; missing: DE, FR; unexpected: BE
  ! L54 fact: fact `round(50)`: round is not exported by surface "default" (undefined: round)
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

### Examples: `go check`, `go compile`

Blocks tagged `go` or `golang` are the adapter's. <!-- fact: Hello().BlockLanguages -->
A block is compiled as a program of its own, inside your module, so it imports your
packages by their import paths and uses your module's dependencies. Nothing is written into
your project: generated files reach the go command through a build overlay.

- **What a block is.** One of three things, told apart by what parses:
  - *a whole file*, when it begins with a package clause. To run, it must be
    `package main` with a `func main`;
  - *declarations*: imports, then functions, types, variables. It is given an empty
    `main` when it declares none;
  - *statements*: imports, then statements, which become the body of `main`. This is the
    usual shape of a snippet in prose.
- **`check`** builds the block and runs it. The proof is what proves a testable example in
  Go: a last comment `// Output:` (or `// Unordered output:`), which what the block prints
  must match, compared as `go test` compares it. A block also fails when it panics or
  exits with a failure, so `if got != want { panic(…) }` is an assertion too; the report
  gives the panic and the line of the block it came from. <!-- test: internal/running/running_test.go > "TestBlocksCompileAndRun" -->
  A block that states no output and calls nothing that ends the program runs, but is
  counted as *weak*.
- **`compile`** builds the block and never runs it. The diagnostics are the Go compiler's
  own, at the lines of the block: an undefined name, a wrong argument, an unused variable
  or import. `check` blocks pass through the same build before they run.
- **Isolation.** Every block is its own package and its own process, started in the
  project root. Blocks share nothing, so each imports what it uses.
- **Time.** A block that outlives `runtime.timeoutMs` is killed with everything it
  started. `runtime.workers` is the number of blocks that run at once. Building is not
  counted: the blocks of a run are built together, once, before any of them runs.
- **Output.** What a block prints is compared or dropped; it never reaches the report.
- **Network.** Host names do not resolve unless the block is tagged `check network`
  (which runs only with `DOCATTEST_NETWORK=1`), so reaching the internet by name fails
  with an error that says why. Names in your hosts file (`localhost`) still resolve, and
  an address written as numbers is dialled as before: the guard replaces
  `net.DefaultResolver`, and code that carries its own resolver is not stopped.

The calls that end a program, which make a block that states no output an assertion:

<!-- cases: weak -->
| block               | weak |
|---------------------|------|
| `fmt.Println(1)`    | yes  |
| `panic("no")`       | no   |
| `log.Fatalf("no")`  | no   |
| `log.Panicln("no")` | no   |
| `os.Exit(1)`        | no   |

### Testable examples in source files

Go already has examples that are checked: the `Example` functions of a `_test.go` file.
Every one in a `sources` file is an attestation, run as `go test` runs it, by its name, in
its package's test program. <!-- test: internal/running/running_test.go > "TestDocExamplesRunAsGoTestRunsThem" -->

```go
// A discount is taken off the price in whole cents.
func ExampleApplyDiscount() {
	price, _ := ApplyDiscount(EUR(999), 50)
	fmt.Println(price)
	// Output: 5.00 EUR
}
```

The claim it defends is its own doc comment. `ExampleApplyDiscount` attests the export
`ApplyDiscount`, and `ExampleMoney_String` attests `Money`. An example that states no
output is compiled and not run, which is what `go test` does with it. The fixture's
`shop_test.go` has the example above, and one that states no output:

```console check cwd=testdata/fixture
$ docattest check shop_test.go
shop_test.go
  ✓ L10 example: printed the output it states
  ✓ L17 example: compiles
...
```

### Facts

A fact expression is a Go expression over the surface's exports, by their own names:

```markdown
A discount is at most `50` percent. <!-- fact: MaxDiscount -->
The carriers: <!-- fact @shipping: Carriers -->
```

It may use the surface's exports, the exported names of the setup package, the builtins,
`snapshot("name")`, `manifest("path")`, and any package of the standard library that is
imported by one word (`strings.ToUpper(DefaultCurrency)`, `len(TaxRates)`): a fact cannot
`import`. An expression has one value; put a function that returns a value and an error
behind a helper in the setup package. A name the surface does not export is an error
(`round is not exported by surface "default"`), which is what keeps a page honest about
what a user can see. <!-- test: internal/running/running_test.go > "TestEvaluateSaysWhyAFactHasNoValue" -->

The page's literal is compared with `fmt.Sprint(value)`: a value is displayed as its
language displays it. So write values the way Go prints them: `true`, `<nil>`, `1.5`,
`[a b]`. <!-- fact: []any{true, nil, 1.5, []string{"a", "b"}} -->
A list on the page is compared with the items of a slice or an array, each through
`fmt.Sprint`. A map's items are its keys, in the order `fmt` prints them:
`<!-- fact: TaxRates -->` above a list checks the keys. <!-- test: internal/running/running_test.go > "TestEvaluateShowsValuesAsGoPrintsThem" -->
`snapshot("name")` is the snapshot's data as `encoding/json` reads JSON into an `any`.

`manifest("go.mod")` is the data of a project file, a manifest, read by the core and given
to the adapter (the [manifests](../../docs/manifests.md) page has the formats, `go.mod`
among them). It is a `map[string]any`, so a field is one index away, as the version at the
top of this page reads it: `manifest("go.mod")["go"]`. <!-- test: internal/running/running_test.go > "TestEvaluateShowsValuesAsGoPrintsThem" -->
A deeper field needs a type assertion (`.(map[string]any)`). The path is relative to the
project root and written out in the fact: a path built at run time is never read.

### Cases tables and named facts: the setup package

```go
// docs/docattest_setup.go
package docs

// Cases are the runners of cases tables, by name. A runner is called once per row with
// the row's cells by column; it returns nil when the row holds, else why it does not.
var Cases = map[string]func(row map[string]string) error{
	"discount": discount,
}

// Columns (optional) are the columns each runner's table must have.
var Columns = map[string][]string{
	"discount": {"price", "percent", "result"},
}

// ErrorMessages is something a fact may use: <!-- fact: ErrorMessages() -->
func ErrorMessages() []string { … }
```

The setup package is an ordinary package of your module: `go build ./...` and
`go vet ./...` see it too. A row is named by the page, its line and its first cell. A
runner that panics fails its row, and the next row still runs.

### Citations

`<!-- test: shop_test.go > "TestTotals" > "rejects big discounts" -->` ties a sentence to
a test. A test is a `Test`, `Fuzz` or `Example` function, titled by its name, or a subtest
started with `t.Run("a literal name", …)`, titled by that name, with the functions and
subtests that enclose it named first, outermost first. A subtest whose name is computed
(a table-driven test's `t.Run(tc.name, …)`) cannot be cited. <!-- test: internal/analysis/analysis_test.go > "TestSubtestsWithComputedNames" -->
Citing a benchmark is an error that says so. The file is read, never run.

## What is never your project's own

The adapter tells the core to leave files and directories named `vendor`, `testdata` and
`node_modules` alone, wherever they are. <!-- fact: Hello().IgnoredNames --> Hidden
directories are left out by the core itself.

## Limits

- One module per project: the generated programs are compiled in it.
- Exports, the index (`where`, `uncovered`) and `impact` are read from syntax, with no
  type information: a method is found through its type, an interface is not followed to
  what implements it, and a name shadowed by a local variable still counts as a use.
- `impact` cuts a package into its top-level names. The constants of one `const (…)`
  declaration are one unit, since `iota` ties them together.
- A fact has one value (see above), and a block cannot mix top-level function
  declarations with loose statements: write a whole file, or use function literals.
- Windows has no process groups here: a block that overruns is killed, and a program it
  started may live on. This path has not been run on Windows.

## Working on this package

[CONTRIBUTING.md](CONTRIBUTING.md) maps the package's sources, says how its tests run, and
how this guide is checked.

## License

[MIT](LICENSE)
