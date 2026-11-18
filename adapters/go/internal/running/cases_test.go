package running

import (
	"fmt"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// processSetup is a setup package whose runners show which rows share a process: `count`
// counts the rows its process has run in a package variable, and `meet` returns only once
// another table runs at the same time.
const processSetup = `package docs

import (
	"fmt"
	"os"
	"time"
)

var rowsRun int

var Cases = map[string]func(row map[string]string) error{
	"count": count,
	"meet":  meet,
}

func count(row map[string]string) error {
	rowsRun++
	switch row["then"] {
	case "exit":
		os.Exit(1)
	case "hang":
		time.Sleep(time.Hour)
	}
	if got := fmt.Sprint(rowsRun); got != row["rows"] {
		return fmt.Errorf("row %s of its process", got)
	}
	return nil
}

func meet(row map[string]string) error {
	if err := os.WriteFile(row["me"], nil, 0o600); err != nil {
		return err
	}
	for {
		if _, err := os.Stat(row["other"]); err == nil {
			return nil
		}
		time.Sleep(10 * time.Millisecond)
	}
}
`

// processTable is a table of the runner, one row for each list of cells.
func processTable(runner string, header []string, rows ...[]string) protocol.Cases {
	table := protocol.Cases{ID: runner, Page: "docs/page.md", Line: 20, Kind: protocol.KindCases, Runner: runner, Header: header}
	for k, cells := range rows {
		values := map[string]string{}
		for c, column := range header {
			values[column] = cells[c]
		}
		table.Rows = append(table.Rows, protocol.CasesRow{Line: 22 + k, Name: cells[0], Values: values})
	}
	return table
}

func openProcessSetup(t *testing.T, timeoutMs float64) *Runner {
	t.Helper()
	return open(t, opening{surfaces: entries("default", "."), timeoutMs: timeoutMs, files: map[string]string{
		"go.mod": "module example.com/m\n\ngo 1.21\n", "m.go": "package m\n", "docs/docattest_setup.go": processSetup,
	}})
}

// statuses are the results as the line of each and its status.
func statuses(results []protocol.Result) string {
	out := []string{}
	for _, result := range results {
		out = append(out, fmt.Sprintf("L%d %s", result.Line, result.Status))
	}
	return strings.Join(out, ", ")
}

func TestATablesRowsShareItsProcess(t *testing.T) {
	header := []string{"case", "rows", "then"}
	results := openProcessSetup(t, 0).Run(nil, []protocol.Cases{
		processTable("count", header,
			[]string{"first", "1", ""}, []string{"second", "2", ""}, []string{"ends it", "", "exit"},
			[]string{"after an exit", "1", ""}, []string{"times out", "", "hang"}, []string{"after a timeout", "1", ""}),
		processTable("count", header, []string{"another table", "1", ""}),
	})
	got := statuses(results)
	// The rows: first, second, one that ends the process, the next, one that times out,
	// the next; and the row of another table.
	want := "L22 pass, L23 pass, L24 fail, L25 pass, L26 fail, L27 pass, L22 pass"
	if got != want {
		t.Fatalf("got %s", got)
	}
}

// sideBySideTimeoutMs bounds a row of TestTablesRunSideBySide: it is reached only when the
// tables run one after the other, so it can be long enough for any load.
const sideBySideTimeoutMs = 60000

func TestTablesRunSideBySide(t *testing.T) {
	header := []string{"case", "me", "other"}
	results := openProcessSetup(t, sideBySideTimeoutMs).Run(nil, []protocol.Cases{
		processTable("meet", header, []string{"one", "one.ran", "two.ran"}),
		processTable("meet", header, []string{"two", "two.ran", "one.ran"}),
	})
	if got := statuses(results); got != "L22 pass, L22 pass" {
		t.Fatalf("got %s", got)
	}
}
