# Facts in Go

What a fact expression may use, how a value is compared with the page, the constants and
variables a fact reads from source, and the docs helpers that keep a package from exporting
anything for its pages. The [guide](../README.md) has the rest.

A fact expression is a Go expression over the surface's exports, by their own names:

```markdown
A discount is at most `50` percent. <!-- fact: MaxDiscount -->
The carriers: <!-- fact @shipping: Carriers -->
```

It may use the surface's exports, its constants and literal variables (below), the
exported names of its docs helper files (below), the exported names of the setup package,
the builtins, `snapshot("name")`, `manifest("path")`, `file("path")`, and any package of
the standard library that is imported by one word (`strings.ToUpper(DefaultCurrency)`,
`len(TaxRates)`): a fact cannot `import`. An expression has one value; put a function that
returns a value and an error behind a helper in the setup package.

A fact that calls a function of the setup package, or reads one of its variables, attests
what that helper uses of the surfaces, at any depth, as a cases table attests what its
runner uses ([cases](cases.md)). <!-- test: internal/analysis/setup_test.go > "TestAFactNamesWhatTheSetupHelpersItCallsUse" -->
So `where` lists the fact under each export the helper reads, `uncovered` does not list
them, and `impact` lists the fact when one of them changes. In this package's fixture, the
setup's `ErrorMessages()` reads `shop.ErrorCodes`, which no page names: a fact that calls
it attests `ErrorCodes`. <!-- test: endtoend/endtoend_test.go > "TestAFactThroughAHelperAttestsWhatTheHelperUses" -->

A fact runs in a program that imports the surface, so a function or a type the surface
does not export is an error that says only constants and literal variables can be read
there (`round is a function that surface "default" does not export; of its unexported
names, a fact reads only constants and literal variables`), which is what keeps a page
honest about what a user can see. <!-- test: internal/running/constants_test.go > "TestOnlyConstantsAndLiteralVariablesAreReadFromSource" -->

Constants are read from source instead, since a service's most documented values, its
port or its default timeout, are often unexported or in its `package main`. A fact may name
any constant declared at the top level of its surface's package, exported or not, and a
surface may be a `package main`, of which only constants and literal variables (below) can
be read. <!-- test: internal/running/constants_test.go > "TestAFactReadsConstantsFromSource" -->
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

## Variables read from source

A fact also reads a variable from source when it is a literal variable: its surface's
package declares it at the top level with a composite literal (of a slice, an array, a map
or a struct) whose elements and keys are constants or such literals, and only reads it.
Such a variable is read as a constant is, exported or not, of a `package main` too, and
nothing of the package runs. <!-- test: internal/running/variables_test.go > "TestAFactReadsLiteralVariablesFromSource" -->
A variable is no literal variable when its value calls anything or names a variable, holds
a pointer, or is missing; a fact that reads it says why (`started is a variable of surface
"shopd" that a fact cannot read from source: its value, time.Now(), is not a composite
literal`). <!-- test: internal/analysis/literalvariables_test.go > "TestALiteralVariableIsOneItsPackageOnlyReads" -->

A package only reads a variable when none of its files (its tests aside, which no program
runs) uses it in a way that may change it. A use is the variable with any index, slice or
field selected from it, and it may change the variable when it is assigned to, incremented,
or ranged into; when its address is taken, also to call a method with a pointer receiver;
or when it shares the variable's storage (a slice, a map, a pointer or an interface) with
anything but `len`, `cap`, the source of `copy`, the `s...` of `append`, or a `range` whose
items share nothing. A value that shares nothing, such as a string, a number, or an array or
struct of them, is copied wherever it goes, so it may go anywhere. The rule is sound and
does not look into functions: passing a slice to `strings.Join` makes it no literal
variable, although `strings.Join` only reads it. <!-- test: internal/analysis/literalvariables_test.go > "TestALiteralVariableIsOneItsPackageOnlyReads" -->
A fact that reads a variable that is reassigned or appended to is an error that says where
(`the package may change it (main.go:53 assigns to it)`). <!-- test: internal/running/variables_test.go > "TestAVariableThatIsNoLiteralVariableIsRefused" -->

The variable is declared in the fact's program with the value its source writes. A type
the program can name (exported, of a package it may import, with exported fields) keeps
its name, so a `map[string]time.Duration` prints `map[read:5s]`. Any other type, such as
one the package declares unexported, is copied with its name and its fields, and without
its methods, as a constant's type of a `package main` loses its `String` method. So a slice
of structs prints as Go prints the struct, `[{CAP_SYS_ADMIN 2} {CAP_NET_RAW 1}]`, and a
fact may use its fields, in a function literal that maps it, for one. <!-- test: internal/running/variables_test.go > "TestAFactReadsLiteralVariablesFromSource" -->
A type that holds a function, a channel, a pointer, an interface with methods, or an
instance of a generic type cannot be copied, and a fact that reads such a variable says so. <!-- test: internal/running/variables_test.go > "TestAVariableThatIsNoLiteralVariableIsRefused" -->
A fact attests the literal variables it reads, as it attests constants, and `impact` lists
a change to one with the facts that read it. <!-- test: endtoend/endtoend_test.go > "TestAFactThroughAHelperAttestsWhatTheHelperUses" -->

## Docs helpers

A setup package cannot read what a surface does not export, so a helper that derives what a
page states from an unexported variable would force an export. A docs helper file is a file
of the surface's package whose build constraint holds only with the tag `docattest`
(`//go:build docattest`): it reads the package's unexported names, and only docattest
builds it. <!-- test: internal/analysis/docsfiles_test.go > "TestADocsHelperFileHoldsOnlyWithTheDocsTag" -->
The adapter lists packages, and builds the programs it generates for facts, cases tables
and `audit`, with that tag. A block of a page is built as `go build` builds it, without the
tag, so a page cannot show code that calls a docs helper. <!-- test: internal/running/docshelpers_test.go > "TestABlockIsBuiltWithoutTheDocsHelpers" -->
A fact calls a docs helper by its name, so a docs helper a fact calls is exported in its
file, which costs nothing since nothing else builds it; one that is not says to export it. <!-- test: internal/running/docshelpers_test.go > "TestAnUnexportedDocsHelperIsRefusedWithWhatToDo" -->
What a docs helper file declares is no export of the surface: `uncovered` never lists it.
A fact that calls a docs helper attests the helper and what it uses of the surfaces, as a
fact that calls a setup helper does. <!-- test: internal/analysis/docsfiles_test.go > "TestADocsHelperIsNoExportAndAttestsWhatItUses" -->
So `impact` lists the fact when anything the helper uses changes, an unexported variable
included. <!-- test: endtoend/endtoend_test.go > "TestAFactThroughAHelperAttestsWhatTheHelperUses" -->
A fact never imports a `package main`, so its docs helpers cannot be called: of a program,
a fact reads constants and literal variables, and a helper goes in an importable package.

## Zero exports for the docs

This package's fixture has a package, `sandbox`, that exports only what its users call. Its
pages state an environment list, a slice of structs, and names that a helper derives from
them, with nothing exported for them:

```go
// sandbox/sandbox.go
package sandbox

type capability struct {
	Name   string
	Reason string
}

var forbiddenCaps = []capability{
	{Name: "CAP_SYS_ADMIN", Reason: "mounts filesystems"},
	{Name: "CAP_NET_RAW", Reason: "forges packets"},
}

var sessionEnv = []string{"LANG=C.UTF-8", "TZ=UTC"}

func Refuses(name string) bool { … } // ranges over forbiddenCaps

func Env(parent []string) []string { return append(parent, sessionEnv...) }
```

```go
// sandbox/sandbox_docattest.go
//go:build docattest

package sandbox

func ForbiddenCapNames() []string { … } // the Name of each of forbiddenCaps

func SessionEnvNames() []string { … } // what comes before the = of each of sessionEnv
```

The environment list and the slice of structs are literal variables, which a fact reads as
they are; the names come from the docs helpers:

```markdown check fixture=testdata/fixture
A sandboxed command's environment adds these variables to its parent's: <!-- fact @sandbox: sessionEnv -->
- `LANG=C.UTF-8`
- `TZ=UTC`

Their names: <!-- fact @sandbox: SessionEnvNames() -->
- `LANG`
- `TZ`

It is refused these capabilities: <!-- fact @sandbox: ForbiddenCapNames() -->
- `CAP_SYS_ADMIN`
- `CAP_NET_RAW`

Go prints them as `[{CAP_SYS_ADMIN mounts filesystems} {CAP_NET_RAW forges packets}]`. <!-- fact @sandbox: forbiddenCaps -->

Each for a reason: <!-- fact @sandbox: func() (reasons []string) { for _, c := range forbiddenCaps { reasons = append(reasons, c.Reason) }; return }() -->
- mounts filesystems
- forges packets
```

`go build ./...` and `go vet ./...` leave the docs helper file out, as the go command
lists it:

```console check cwd=testdata/fixture
$ go list -f '{{.GoFiles}} {{.IgnoredGoFiles}}' ./sandbox
[sandbox.go] [sandbox_docattest.go]
$ go list -tags docattest -f '{{.GoFiles}}' ./sandbox
[sandbox.go sandbox_docattest.go]
```

The sandbox's exports are `Refuses` and `Env`, which its pages attest, so `uncovered`
lists nothing of it. <!-- test: endtoend/endtoend_test.go > "TestTheIndexAndTheAudit" -->
