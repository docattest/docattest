package running

import (
	"encoding/json"
	"fmt"
	"go/parser"
	"regexp"
	"strings"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// A fact expression is a Go expression. It is compiled in a file that imports its
// surface with a dot, so the surface's exports, and the exported names of its docs helper
// files, are in scope by their own names; the setup package's exports are in scope the
// same way; and a package of the standard library is there by its name. Of what a program
// importing the surface cannot use, the constants and the literal variables are declared
// in the fact's own function, read from source (sourcevalues.go).

var undefined = regexp.MustCompile(`^undefined: (\w+)$`)

// Evaluate answers `evaluate`: an Evaluation for every fact, by id.
func (r *Runner) Evaluate(facts []protocol.Fact) map[string]protocol.Evaluation {
	out := map[string]protocol.Evaluation{}
	groups := map[string][]protocol.Fact{}
	order := []string{}
	for _, fact := range facts {
		if refusal := r.refusal(fact); refusal != "" {
			out[fact.ID] = protocol.Failed(refusal)
			continue
		}
		if groups[fact.Surface] == nil {
			order = append(order, fact.Surface)
		}
		groups[fact.Surface] = append(groups[fact.Surface], fact)
	}
	// Each surface's facts are a program of their own; the programs are built and run
	// side by side.
	var all sync.WaitGroup
	answers := make([]map[string]protocol.Evaluation, len(order))
	for k, name := range order {
		all.Add(1)
		go func() {
			defer all.Done()
			answers[k] = r.evaluateGroup(r.project.SurfaceNamed(name), groups[name])
		}()
	}
	all.Wait()
	for _, group := range answers {
		for id, evaluation := range group {
			out[id] = evaluation
		}
	}
	return out
}

// refusal is why a fact cannot be compiled at all, or "".
func (r *Runner) refusal(fact protocol.Fact) string {
	noSurfaces := !r.project.HasSurfaces() && fact.Surface == "default"
	if !noSurfaces {
		if missing := r.project.NotFound(fact.Surface); missing != "" {
			return missing
		}
	}
	if _, err := parser.ParseExpr(fact.Expression); err != nil {
		return fmt.Sprintf("cannot parse fact expression `%s`: %v", fact.Expression, err)
	}
	if noSurfaces {
		return ""
	}
	of := r.project.SurfaceNamed(fact.Surface)
	importable, helpers := r.project.Importable(of), r.project.SetupExports()
	for _, name := range language.Names(fact.Expression) {
		if importable[name] && helpers[name] {
			return fmt.Sprintf("fact `%s`: %q is both an export of surface %q and of the setup package; rename the one in the setup package",
				fact.Expression, name, fact.Surface)
		}
	}
	for _, used := range r.sourceNames(of, fact.Expression) {
		if used.what != analysis.Constant && used.what != analysis.Variable {
			return fmt.Sprintf("fact `%s`: %s", fact.Expression, r.unreadable(used, of))
		}
	}
	return ""
}

// unreadable says why a fact cannot read a function or a type of its surface that a
// program importing it cannot use.
func (r *Runner) unreadable(used sourceName, of *analysis.Surface) string {
	switch {
	case r.project.IsProgram(of):
		return fmt.Sprintf("%s is a %s of surface %q, a package main; of a package main, a fact reads only constants and literal variables",
			used.name, used.what, of.Name)
	case r.project.DocsHelpers(of)[used.name] != "":
		return fmt.Sprintf("%s is a %s of a docs helper file of surface %q that is not exported; export it there, since only docattest builds that file",
			used.name, used.what, of.Name)
	}
	return fmt.Sprintf("%s is a %s that surface %q does not export; of its unexported names, a fact reads only constants and literal variables",
		used.name, used.what, of.Name)
}

// evaluateGroup evaluates the facts of one surface in one program.
func (r *Runner) evaluateGroup(of *analysis.Surface, facts []protocol.Fact) map[string]protocol.Evaluation {
	var values sourceValues
	out := map[string]protocol.Evaluation{}
	if of != nil {
		facts, values, out = r.withSourceValues(of, facts)
	}
	if len(facts) == 0 {
		return out
	}
	labels := make([]string, len(facts))
	for k := range facts {
		labels[k] = fmt.Sprintf("docattest_f%d.go", k)
	}
	program := r.buildProgram(labels, func(live []bool) string { return r.factsMain(of, facts, labels, live, values) })
	kept := []protocol.Fact{}
	surfaceName := "default"
	if of != nil {
		surfaceName = of.Name
	}
	for k, fact := range facts {
		label := fmt.Sprintf("fact `%s`", fact.Expression)
		switch complaint, dropped := program.dropped[k]; {
		case dropped:
			out[fact.ID] = protocol.Failed(label + ": " + notExported(complaint, of))
		case program.binary == "":
			out[fact.ID] = protocol.Failed(fmt.Sprintf("surface %q failed to load: %s", surfaceName, program.failure))
		default:
			kept = append(kept, fact)
		}
	}
	if len(kept) == 0 {
		return out
	}
	answers, failed := r.serve(program.binary, nil, len(kept))
	for k, fact := range kept {
		if failed != "" {
			out[fact.ID] = protocol.Failed(failed)
			continue
		}
		out[fact.ID] = r.evaluation(fact, answers[k])
	}
	return out
}

// notExported words the compiler's `undefined: X` as what it means on a page.
func notExported(complaint string, of *analysis.Surface) string {
	if found := undefined.FindStringSubmatch(complaint); found != nil {
		if of == nil {
			return fmt.Sprintf("%s is not in scope (%s)", found[1], complaint)
		}
		return fmt.Sprintf("%s is not exported by surface %q (%s)", found[1], of.Name, complaint)
	}
	return complaint
}

func (r *Runner) evaluation(fact protocol.Fact, answer line) protocol.Evaluation {
	label := fmt.Sprintf("fact `%s`", fact.Expression)
	switch {
	case answer.Timeout:
		return protocol.Failed(label + " " + r.timeoutText())
	case answer.Panic != "":
		return protocol.Failed(label + " panicked: " + answer.Panic)
	case answer.Lost != "":
		return protocol.Failed(label + ": " + answer.Lost)
	}
	var value protocol.Observation
	if err := json.Unmarshal(answer.Value, &value); err != nil {
		return protocol.Failed(label + ": the program returned no value")
	}
	return protocol.Evaluation{OK: true, Value: &value}
}

// factsMain writes the main of a facts program for the facts still in it. A fact's
// function starts with what it reads from source, on the fact's first line.
func (r *Runner) factsMain(of *analysis.Surface, facts []protocol.Fact, labels []string, live []bool, values sourceValues) string {
	imports := newImports()
	fields := decodeManifestFields(r.options.Fields)
	var body strings.Builder
	for k, fact := range facts {
		if !live[k] {
			continue
		}
		r.factImports(imports, of, fact.Expression)
		declarations := ""
		if of != nil {
			declarations = values.prelude(r.sourceRead(of, fact.Expression), imports)
		}
		expr := typedFieldCalls(strings.TrimSpace(fact.Expression), fields)
		fmt.Fprintf(&body, "\t\tfunc() any {\n//line %s:1\n\t\t\t%sreturn (%s)\n\t\t},\n",
			labels[k], declarations, expr)
	}
	return "package main\n\n" + imports.block() +
		"\nfunc main() {\n\tdocattestFacts([]func() any{\n" + body.String() + "\t})\n}\n"
}

// factImports adds what an expression needs imported: its surface and the setup
// package, each with a dot, when it uses a name of theirs that can be imported; and
// standard packages.
func (r *Runner) factImports(imports *programImports, of *analysis.Surface, expression string) {
	helpers := r.project.SetupExports()
	var exported map[string]bool
	importable := false
	if of != nil {
		exported = r.project.Importable(of)
		importable = !r.project.IsProgram(of)
	}
	for _, name := range language.Names(expression) {
		if exported[name] && importable {
			imports.add(".", of.Import)
		}
		if helpers[name] {
			imports.add(".", r.project.SetupImport())
		}
	}
	for _, name := range language.Qualifiers(expression) {
		if r.standardPackage(name) {
			imports.add(name, name)
		}
	}
}
