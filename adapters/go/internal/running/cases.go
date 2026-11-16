package running

import (
	"encoding/json"
	"fmt"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// A cases table is run by a runner of the setup package (see analysis), row by row, in
// one program that every table of the project shares.

// casesJob is a table as its program is told it (the harness's docattestTable).
type casesJob struct {
	Runner string              `json:"runner"`
	Setup  string              `json:"setup"`
	Line   int                 `json:"line"`
	Header []string            `json:"header"`
	Rows   []protocol.CasesRow `json:"rows"`
}

// runCases runs a cases table: a result for each row at the row's line, or one error
// at the table's line when the table cannot be run at all.
func (r *Runner) runCases(table protocol.Cases) []protocol.Result {
	at := fmt.Sprintf("cases %q at L%d", table.Runner, table.Line)
	refuse := func(message string) []protocol.Result {
		return []protocol.Result{verdict{Status: protocol.StatusError, Message: message}.result(table.Page, table.Line, protocol.KindCases)}
	}
	switch {
	case r.project.Setup == "":
		return refuse(at + `: no setup package is configured ("setup" in docattest.config.json)`)
	case !r.project.SetupFound():
		return refuse(fmt.Sprintf("%s: setup package %s not found", at, r.project.Setup))
	case len(table.Rows) == 0:
		return []protocol.Result{}
	}
	program := r.casesProgram()
	if program.binary == "" {
		return refuse(fmt.Sprintf("%s: setup package %s failed to load: %s", at, r.project.Setup, program.failure))
	}
	told := casesJob{table.Runner, r.project.Setup, table.Line, table.Header, table.Rows}
	answers, failed := r.serve(program.binary, &told, len(table.Rows))
	if failed != "" {
		return refuse(failed)
	}
	out := make([]protocol.Result, len(table.Rows))
	for k, row := range table.Rows {
		name := fmt.Sprintf("%s:%d %s", table.Page, row.Line, row.Name)
		out[k] = r.rowVerdict(name, answers[k]).result(table.Page, row.Line, protocol.KindCases)
	}
	return out
}

// casesProgram is the program that runs the rows of any table of the project: it is
// the same for every table, so a session builds it once.
func (r *Runner) casesProgram() built {
	r.cases.once.Do(func() {
		r.cases.program = r.buildProgram(nil, func([]bool) string { return r.casesMain() })
	})
	return r.cases.program
}

// casesMain writes the main of a cases program: the setup package's runners, and the
// columns they declare when it declares any.
func (r *Runner) casesMain() string {
	columns := "nil"
	if r.project.SetupExports()["Columns"] {
		columns = "setup.Columns"
	}
	return fmt.Sprintf("package main\n\nimport setup %q\n\nfunc main() {\n\tdocattestCases(setup.Cases, %s)\n}\n",
		r.project.SetupImport(), columns)
}

// rowVerdict judges one row by its program's answer: the runner's error, or "".
func (r *Runner) rowVerdict(name string, answer line) verdict {
	switch {
	case answer.Timeout:
		return fail(name + ": " + r.timeoutText())
	case answer.Panic != "":
		return fail(name + ": panic: " + answer.Panic)
	case answer.Lost != "":
		return fail(name + ": " + answer.Lost)
	}
	var why string
	if len(answer.Value) > 0 {
		if err := json.Unmarshal(answer.Value, &why); err != nil {
			return fail(fmt.Sprintf("%s: the runner's answer %s is not text: %v", name, answer.Value, err))
		}
	}
	if why != "" {
		return fail(name + ": " + why)
	}
	return pass(name)
}
