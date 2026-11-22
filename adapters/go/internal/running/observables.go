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
// helpers return. Of a package main, which nothing can import, only the exported
// constants are offered, read from source. A surface that cannot be read is the
// project's mistake.
func (r *Runner) Observables(name string) ([]protocol.Observable, error) {
	if missing := r.project.NotFound(name); missing != "" {
		return nil, protocol.UserError("docattest: " + missing)
	}
	of := r.project.SurfaceNamed(name)
	roots, err := r.roots(of)
	if err != nil {
		return nil, protocol.UserError(fmt.Sprintf("docattest: surface %q failed to load: %v", name, err))
	}
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
		message := "docattest: surface %q failed to load: %s"
		return nil, protocol.UserError(fmt.Sprintf(message, name, program.failure))
	}
	answers, failed := r.serve(program.binary, nil, 1)
	if why := unread(answers, failed, r.timeoutText()); why != "" {
		return nil, protocol.UserError(fmt.Sprintf("docattest: surface %q could not be read: %s", name, why))
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

// root is where `observables` starts reading: the fact expression that reads it, and
// where the program reads it from: the surface, the setup package, or a constant or
// literal variable read from source.
type root struct {
	expression string
	setup      bool
	constant   *sourceConstant
	variable   *sourceVariable
}

// roots are a surface's readable values: exported values, `//go:build docattest` docs
// helper values and zero-arg functions, constants and literal variables read from source
// (unexported or in package main), and on the default surface, the setup package's helpers.
func (r *Runner) roots(of *analysis.Surface) ([]root, error) {
	out := r.importableRoots(of)
	source, err := r.sourceValueRoots(of)
	if err != nil {
		return nil, err
	}
	out = append(out, source...)
	if of.Name != defaultSurface {
		return out, nil
	}
	taken := r.project.Importable(of)
	sourceOnly := r.project.SourceOnly(of)
	for _, name := range r.project.SetupHelpers() {
		if !taken[name] && sourceOnly[name] == "" {
			out = append(out, root{expression: name + "()", setup: true})
		}
	}
	return out, nil
}

// importableRoots are the exported values and `//go:build docattest` docs helpers of an
// importable surface (never a package main).
func (r *Runner) importableRoots(of *analysis.Surface) []root {
	if r.project.IsProgram(of) {
		return []root{}
	}
	out := []root{}
	for _, name := range r.project.ExportedValues(of) {
		out = append(out, root{expression: name})
	}
	for _, name := range r.project.DocsHelperValues(of) {
		out = append(out, root{expression: name})
	}
	for _, name := range r.project.DocsHelperFunctions(of) {
		out = append(out, root{expression: name + "()"})
	}
	return out
}

// sourceValueRoots are a surface's constants and literal variables read from source
// (unexported names of any package, and all names of a package main). Variables that are
// not literal variables are skipped quietly.
func (r *Runner) sourceValueRoots(of *analysis.Surface) ([]root, error) {
	names := r.project.SourceOnlyValues(of)
	if len(names) == 0 {
		return nil, nil
	}
	sourceOnly := r.project.SourceOnly(of)
	constNames := []string{}
	for _, name := range names {
		if sourceOnly[name] == analysis.Constant {
			constNames = append(constNames, name)
		}
	}
	constants := map[string]sourceConstant{}
	if len(constNames) > 0 {
		var err error
		if constants, err = r.readConstants(of, constNames); err != nil {
			return nil, err
		}
	}
	_, checkedErr := r.project.Checked(of)
	out := []root{}
	for _, name := range names {
		if read, ok := constants[name]; ok {
			out = append(out, root{expression: name, constant: &read})
		} else if checkedErr == nil && sourceOnly[name] == analysis.Variable {
			if read, err := r.readSourceVariable(of, name); err == nil {
				out = append(out, root{expression: name, variable: &read})
			}
		}
	}
	return out, nil
}

// observablesMain writes the main of a program that reads the roots still in it.
func (r *Runner) observablesMain(of *analysis.Surface, roots []root, labels []string, live []bool) string {
	imports := newImports()
	var body strings.Builder
	for k, root := range roots {
		if !live[k] {
			continue
		}
		read := "return surface." + root.expression
		switch {
		case root.setup:
			read = "return setup." + root.expression
			imports.add("setup", r.project.SetupImport())
		case root.constant != nil:
			read = root.constant.declaration(imports) + "; return " + root.expression
		case root.variable != nil:
			values := sourceValues{variables: map[string]sourceVariable{root.expression: *root.variable}}
			read = values.prelude([]string{root.expression}, imports) + "return " + root.expression
		default:
			imports.add("surface", of.Import)
		}
		fmt.Fprintf(&body, "\t\t{%q, func() any {\n//line %s:1\n\t\t\t%s\n\t\t}},\n", root.expression, labels[k], read)
	}
	return "package main\n\n" + imports.block() +
		"\nfunc main() {\n\tdocattestObservables([]docattestRoot{\n" + body.String() + "\t})\n}\n"
}
