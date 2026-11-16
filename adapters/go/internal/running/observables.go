package running

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// defaultSurface is the surface a fact without `@` reads (PROTOCOL.md, `observables`).
const defaultSurface = "default"

// Observables answers `observables`: the values a surface offers, which are its
// exported constants and variables and what lies below them, each with the fact
// expression that reads it; on the default surface, also what the setup package's
// helpers return. A surface that cannot be read is the project's mistake.
func (r *Runner) Observables(name string) ([]protocol.Observable, error) {
	if missing := r.project.NotFound(name); missing != "" {
		return nil, protocol.UserError("docattest: audit: " + missing)
	}
	of := r.project.SurfaceNamed(name)
	roots := r.roots(of)
	labels := make([]string, len(roots))
	for k := range roots {
		labels[k] = fmt.Sprintf("docattest_o%d.go", k)
	}
	out := []protocol.Observable{}
	if len(roots) == 0 {
		return out, nil
	}
	program := r.buildProgram(labels, func(live []bool) string { return r.observablesMain(of, roots, labels, live) })
	if program.binary == "" {
		if program.failure == "" {
			return out, nil
		}
		message := "docattest: audit: surface %q failed to load: %s"
		return nil, protocol.UserError(fmt.Sprintf(message, name, program.failure))
	}
	answers, failed := r.serve(program.binary, nil, 1)
	if why := unread(answers, failed, r.timeoutText()); why != "" {
		return nil, protocol.UserError(fmt.Sprintf("docattest: audit: surface %q could not be read: %s", name, why))
	}
	if err := json.Unmarshal(answers[0].Value, &out); err != nil {
		return nil, fmt.Errorf("surface %q: what its program read is not a list of observables: %w", name, err)
	}
	for k := range out {
		out[k].Surface = name
	}
	return out, nil
}

// unread is why a program's one item has no value, or "".
func unread(answers []line, failed, timedOut string) string {
	switch {
	case failed != "":
		return failed
	case answers[0].Timeout:
		return timedOut
	case answers[0].Panic != "":
		return "panic: " + answers[0].Panic
	}
	return answers[0].Lost
}

// root is where `observables` starts reading: the fact expression that reads it, and the
// package the program reads it from.
type root struct {
	expression string
	setup      bool
}

// roots are a surface's exported constants and variables and, on the default surface, a
// call of each setup helper, unless the surface exports its name: a fact could not use it.
func (r *Runner) roots(of *analysis.Surface) []root {
	out := []root{}
	for _, name := range r.project.ExportedValues(of) {
		out = append(out, root{expression: name})
	}
	if of.Name != defaultSurface {
		return out
	}
	exported := r.project.Exports(of)
	for _, name := range r.project.SetupHelpers() {
		if !exported[name] {
			out = append(out, root{expression: name + "()", setup: true})
		}
	}
	return out
}

// observablesMain writes the main of a program that reads the roots still in it.
func (r *Runner) observablesMain(of *analysis.Surface, roots []root, labels []string, live []bool) string {
	imports := map[string]string{}
	var body strings.Builder
	for k, root := range roots {
		if !live[k] {
			continue
		}
		read := "surface." + root.expression
		if root.setup {
			read, imports[r.project.SetupImport()] = "setup."+root.expression, "setup"
		} else {
			imports[of.Import] = "surface"
		}
		fmt.Fprintf(&body, "\t\t{%q, func() any {\n//line %s:1\n\t\t\treturn %s\n\t\t}},\n", root.expression, labels[k], read)
	}
	return "package main\n\n" + importBlock(imports) +
		"\nfunc main() {\n\tdocattestObservables([]docattestRoot{\n" + body.String() + "\t})\n}\n"
}
