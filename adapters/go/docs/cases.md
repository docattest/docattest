# Cases tables and the setup package in Go

The setup package holds the runners of cases tables and what facts may use by name. The
[guide](../README.md) has the rest.

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

A runner gets each cell's text as the page writes it, code spans included: a cell
`` `fmt.Println(1)` `` reaches it with its backticks
([docs/cases.md](../../../docs/cases.md#a-runner-receives-each-cell-as-written)).
The package `github.com/docattest/docattest/adapters/go/cells` takes the code out:
`cells.CodeSpans` gives the content of every code span in a cell, in order, as a fact over
the cell's column compares it. <!-- test: endtoend/cells_test.go > "TestCodeSpansAreWhatAColumnFactCompares" -->

```go check
import (
	"fmt"

	"github.com/docattest/docattest/adapters/go/cells"
)

fmt.Printf("%q\n", cells.CodeSpans("`FOO=1` `BAR=2`"))
fmt.Printf("%q\n", cells.CodeSpans("``a`b`` or `` `c` ``"))
fmt.Printf("%q\n", cells.CodeSpans("no code"))
// Output:
// ["FOO=1" "BAR=2"]
// ["a`b" "`c`"]
// []
```

A span opens with a run of backticks and closes at the next run of as many, and the white
space around its content is dropped. As CommonMark has it, a backtick escaped with a
backslash opens no span, and neither does one inside an HTML tag or comment, an autolink,
or a link's destination or title. <!-- test: cells/codespans_test.go > "TestCodeSpans" -->
The `weak` runner of the [examples](examples.md#blocks-go-check-go-compile) reads the one
block of its cell so:

```go
spans := cells.CodeSpans(row["block"])
if len(spans) != 1 {
	return fmt.Errorf("a block is one code span, not %q", row["block"])
}
```

`go get github.com/docattest/docattest/adapters/go/cells` adds this module to yours. The
package imports only the standard library, so the go command downloads this module's
source and none of the cores the commands carry. <!-- test: endtoend/cells_test.go > "TestCellsImportsOnlyTheStandardLibrary" -->
A module whose `go` line is lower than this module's, `1.23`, has it raised to that. <!-- fact: manifest("go.mod")["go"] -->

A row is named by the page, its line and its first cell. A runner that panics fails its
row, and the next row still runs. A table attests the exports its runner uses (what
`where` and `uncovered` count): in the runner's own code, and in the setup package's
functions, methods, variables and constants it uses, at any depth. <!-- test: internal/analysis/setup_test.go > "TestARunnerNamesWhatItsHelpersUse" -->

A table's rows run one after another, in the page's order, in one process. Package-level
state, and anything a row starts and does not stop (goroutines, servers, signal handlers,
a reaper that waits on any child), is still there for the rows after it, so a runner stops
what it starts before it returns. A row that runs out of time or ends the process is the
last of its process: the next row starts a fresh one. <!-- test: internal/running/cases_test.go > "TestATablesRowsShareItsProcess" -->
Tables never share a process, and run side by side, with each other and with blocks:
`runtime.workers` processes at once in all. <!-- test: internal/running/cases_test.go > "TestTablesRunSideBySide" -->

The setup package is an ordinary package of your module: `go build ./...` and
`go vet ./...` see it too. The adapter reads it as the go command builds it with one tag,
`docattest`, the tag of [docs helper files](facts.md#docs-helpers): a file behind any other
tag, such as `//go:build integration`, is left out, and a setup package with no other file
is not found. <!-- test: internal/analysis/setup_test.go > "TestASetupFileBehindABuildTagIsLeftOut" -->
So a setup package whose files are all behind `//go:build docattest` is read, and
`go build ./...` and `go vet ./...` leave it out. <!-- test: internal/analysis/setup_test.go > "TestASetupPackageBehindTheDocsTagIsRead" -->
No program of yours links it unless one imports it, so it changes no binary. A CI rule
that rebuilds on any change to `**/*.go` still fires on a change made for the pages only:
keep the setup package beside the pages (`docs/`) and leave that directory out of such
rules (`paths-ignore: ["docs/**"]` in a GitHub Actions workflow).
