package program

import (
	"fmt"
	"sort"
	"strings"
)

// docattestTable is a cases table as the adapter sends it.
type docattestTable struct {
	Runner string   `json:"runner"`
	Setup  string   `json:"setup"`
	Line   int      `json:"line"`
	Header []string `json:"header"`
	Rows   []struct {
		Values map[string]string `json:"values"`
	} `json:"rows"`
}

// docattestRunner runs one row of a table: nil when the row holds, an error that says
// why when it does not.
type docattestRunner = func(row map[string]string) error

// docattestCases serves the rows of the job's table with the runner the setup package
// names for it: each item is "" for a row that holds, else why it does not. `columns`
// (which may be nil) are the columns a runner says its table must have.
func docattestCases(cases map[string]docattestRunner, columns map[string][]string) {
	first := docattestStart()
	table := docattestTold.Cases
	run, found := cases[table.Runner]
	if refusal := docattestRefusal(table, cases, found, columns[table.Runner]); refusal != "" {
		docattestWrite(docattestLine{Failed: refusal})
		return
	}
	docattestServe(first, len(table.Rows), func(k int) any {
		if err := run(table.Rows[k].Values); err != nil {
			return err.Error()
		}
		return ""
	})
}

// docattestRefusal is why a table cannot be run at all, or "".
func docattestRefusal(table docattestTable, cases map[string]docattestRunner, found bool, columns []string) string {
	if !found {
		known := make([]string, 0, len(cases))
		for name := range cases {
			known = append(known, name)
		}
		sort.Strings(known)
		listed := strings.Join(known, ", ")
		if listed == "" {
			listed = "none"
		}
		return fmt.Sprintf("cases runner %q at L%d is not in Cases of %s (known: %s)",
			table.Runner, table.Line, table.Setup, listed)
	}
	missing := []string{}
	for _, column := range columns {
		if !docattestHas(table.Header, column) {
			missing = append(missing, fmt.Sprintf("%q", column))
		}
	}
	if len(missing) == 0 {
		return ""
	}
	noun := "column"
	if len(missing) > 1 {
		noun = "columns"
	}
	return fmt.Sprintf("cases %q at L%d: the table is missing %s %s declared by the runner",
		table.Runner, table.Line, noun, strings.Join(missing, ", "))
}

func docattestHas(list []string, wanted string) bool {
	for _, item := range list {
		if item == wanted {
			return true
		}
	}
	return false
}
