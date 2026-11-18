# Facts in Go

What a fact expression may use, how a value is compared with the page, and the constants a
fact reads from source. The [guide](../README.md) has the rest.

A fact expression is a Go expression over the surface's exports, by their own names:

```markdown
A discount is at most `50` percent. <!-- fact: MaxDiscount -->
The carriers: <!-- fact @shipping: Carriers -->
```

It may use the surface's exports and its constants (below), the exported names of the
setup package, the builtins, `snapshot("name")`, `manifest("path")`, `file("path")`, and
any package of the standard library that is imported by one word
(`strings.ToUpper(DefaultCurrency)`, `len(TaxRates)`): a fact cannot `import`. An
expression has one value; put a function that returns a value and an error behind a helper
in the setup package.

A fact runs in a program that imports the surface, so a function, variable or type the
surface does not export is an error that says only constants can be read there
(`round is a function that surface "default" does not export; of its unexported names, a
fact reads only constants`), which is what keeps a page honest about what a user can see. <!-- test: internal/running/constants_test.go > "TestOnlyConstantsAreReadFromSource" -->

Constants are read from source instead, since a service's most documented values, its
port or its default timeout, are often unexported or in its `package main`. A fact may name
any constant declared at the top level of its surface's package, exported or not, and a
surface may be a `package main`, of which only constants can be read. <!-- test: internal/running/constants_test.go > "TestAFactReadsConstantsFromSource" -->
The package and what it imports are type-checked from source with `go/types`, as the go
command lists them, and the fact gets the value the type checker gives the constant:
nothing imports the package, and nothing of it runs. <!-- test: internal/running/constants_test.go > "TestNothingOfThePackageRuns" -->
Here `shopd` is `./cmd/shopd`, a `package main`:

```go
// cmd/shopd/main.go
package main

const listenPort = 8080
const requestTimeout = 30 * time.Second
const region = "nl"
const logLevel verbosity = 2 // verbosity is an int with a String method
```

```markdown check fixture=testdata/fixture
The server listens on port `8080`. <!-- fact @shopd: listenPort -->
A request is cancelled after `30s`, <!-- fact @shopd: requestTimeout -->
which is `30` seconds. <!-- fact @shopd: requestTimeout.Seconds() -->
A basket that names no region is taxed as `NL`. <!-- fact @shopd: strings.ToUpper(region) -->
Logging starts at level `2`. <!-- fact @shopd: logLevel -->
```

A constant keeps its type where the fact's program can name it, so `requestTimeout` is a
`time.Duration` and prints as one. A type that a `package main` declares, an unexported
one, or one in an `internal` directory the program may not import cannot be named, so the
constant has the underlying type: `logLevel` is the int `2`, and the `String` method of
`verbosity` is not called. An untyped constant stays untyped, so `1.0 / 3` is still a
float. <!-- test: internal/running/constants_test.go > "TestAConstantKeepsItsKindAndItsType" -->
When the package does not compile, a fact that reads one of its constants says so, and the
surface's other facts still run. <!-- test: internal/running/constants_test.go > "TestAPackageThatDoesNotCompileFailsOnlyTheFactsThatReadIt" -->
Export data is never read, so the project's go command may be newer than the Go the
adapter was built with (a `toolchain` line in `go.mod` chooses it) and write export data
the adapter could not read. <!-- test: internal/analysis/load_test.go > "TestPackagesAreCheckedFromSourceWithoutExportData" -->
A `go` line or a go command newer than that Go is no error of the package, although
`go/types` reports one for such a `go` line. <!-- test: internal/analysis/load_test.go > "TestAGoNewerThanTheAdapterIsNoErrorOfThePackage" -->
A fact attests the constants it reads, exported or not, as `where` shows; `uncovered` lists
only exports. <!-- test: internal/analysis/analysis_test.go > "TestSymbols" -->
So `impact` lists a change to such a constant as it lists a change to an export, with the
facts that read it: an unexported constant, or any constant of a `package main`. <!-- test: endtoend/endtoend_test.go > "TestImpactOfAConstantReadFromSource" -->

The page's literal is compared with `fmt.Sprint(value)`: a value is displayed as its
language displays it. So write values the way Go prints them: `true`, `<nil>`, `1.5`,
`[a b]`. <!-- fact: []any{true, nil, 1.5, []string{"a", "b"}} -->
A list on the page is compared with the items of a slice or an array, each through
`fmt.Sprint`. A map's items are its keys, in the order `fmt` prints them:
`<!-- fact: TaxRates -->` above a list checks the keys, as on
[the guide's page](../README.md#a-page). <!-- test: internal/running/running_test.go > "TestEvaluateShowsValuesAsGoPrintsThem" -->
`snapshot("name")` is the snapshot's data as `encoding/json` reads JSON into an `any`,
but for its numbers, which are read as a data block's values are (below).

A fact with `field=` checks one value of a JSON, TOML or YAML block
([facts](../../../docs/facts.md#a-fact-can-check-one-field-of-a-data-block)), which is
written as its format writes values. The adapter shows the block's value through
`fmt.Sprint` too, so it compares with the fact's as the same value: `null` is `<nil>`, a
number written as an integer is an `int64`, as a constant of the same value is (`1048576`,
not the `1.048576e+06` of a `float64`), and any other number a `float64` (`1.0` is `1`). <!-- test: internal/language/language_test.go > "TestDisplayShowsADataBlocksValuesAsGoDoes" -->

`manifest("go.mod")` is the data of a project file, a manifest, read by the core and given
to the adapter (the [manifests](../../../docs/manifests.md) page has the formats, `go.mod`
among them). It is a `map[string]any`, so a field is one index away, as the version at the
top of the [guide](../README.md#install) reads it: `manifest("go.mod")["go"]`. <!-- test: internal/running/running_test.go > "TestEvaluateShowsValuesAsGoPrintsThem" -->
A deeper field needs a type assertion (`.(map[string]any)`). The path is relative to the
project root and written out in the fact: a path built at run time is never read.
A number of a manifest or a snapshot is as Go holds a value written so, as a data block's
is: of the display cases in this adapter's conformance kit, the fourth is `1048576`, <!-- fact: manifest("conformance.json")["display"].([]any)[3].(map[string]any)["value"] -->
an `int64`, where the `float64` that `encoding/json` reads would print `1.048576e+06`;
a number not written as an integer is a `float64`. <!-- test: internal/running/running_test.go > "TestNumbersOfSnapshotsAndManifestsAreAsGoHoldsThem" -->

`file("path")` is the text of any other project file, a `string`, read by the same rules,
for the `strings` and `regexp` packages:
``regexp.MustCompile(`USER (\d+)`).FindStringSubmatch(file("Dockerfile"))[1]`` reads the
user a `Dockerfile` runs as. This module's license is read so: it is `MIT`. <!-- fact: regexp.MustCompile(`^(\w+) License`).FindStringSubmatch(file("LICENSE"))[1] -->
