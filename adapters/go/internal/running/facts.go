package running

import (
	"encoding/json"
	"fmt"
	"go/parser"
	"regexp"
	"sort"
	"strings"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// A fact expression is a Go expression. It is compiled in a file that imports its
// surface with a dot, so the surface's exports are in scope by their own names and
// nothing else of the package is; the setup package's exports are in scope the same
// way; and a package of the standard library is there by its name.

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
	if missing := r.project.NotFound(fact.Surface); missing != "" {
		return missing
	}
	if _, err := parser.ParseExpr(fact.Expression); err != nil {
		return fmt.Sprintf("cannot parse fact expression `%s`: %v", fact.Expression, err)
	}
	exported, helpers := r.project.Exports(r.project.SurfaceNamed(fact.Surface)), r.project.SetupExports()
	for _, name := range language.Names(fact.Expression) {
		if exported[name] && helpers[name] {
			return fmt.Sprintf("fact `%s`: %q is both an export of surface %q and of the setup package; rename the one in the setup package",
				fact.Expression, name, fact.Surface)
		}
	}
	return ""
}

// evaluateGroup evaluates the facts of one surface in one program.
func (r *Runner) evaluateGroup(of *analysis.Surface, facts []protocol.Fact) map[string]protocol.Evaluation {
	out := map[string]protocol.Evaluation{}
	labels := make([]string, len(facts))
	for k := range facts {
		labels[k] = fmt.Sprintf("docattest_f%d.go", k)
	}
	program := r.buildProgram(labels, func(live []bool) string { return r.factsMain(of, facts, labels, live) })
	kept := []protocol.Fact{}
	for k, fact := range facts {
		label := fmt.Sprintf("fact `%s`", fact.Expression)
		switch complaint, dropped := program.dropped[k]; {
		case dropped:
			out[fact.ID] = protocol.Failed(label + ": " + notExported(complaint, of))
		case program.binary == "":
			out[fact.ID] = protocol.Failed(fmt.Sprintf("surface %q failed to load: %s", of.Name, program.failure))
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

// factsMain writes the main of a facts program for the facts still in it.
func (r *Runner) factsMain(of *analysis.Surface, facts []protocol.Fact, labels []string, live []bool) string {
	imports := map[string]string{}
	var body strings.Builder
	for k, fact := range facts {
		if !live[k] {
			continue
		}
		r.factImports(imports, of, fact.Expression)
		fmt.Fprintf(&body, "\t\tfunc() any {\n//line %s:1\n\t\t\treturn (%s)\n\t\t},\n", labels[k], strings.TrimSpace(fact.Expression))
	}
	return "package main\n\n" + importBlock(imports) +
		"\nfunc main() {\n\tdocattestFacts([]func() any{\n" + body.String() + "\t})\n}\n"
}

// factImports adds what an expression needs imported: its surface and the setup
// package, each with a dot, when it uses a name of theirs; and standard packages.
func (r *Runner) factImports(imports map[string]string, of *analysis.Surface, expression string) {
	exported, helpers := r.project.Exports(of), r.project.SetupExports()
	for _, name := range language.Names(expression) {
		if exported[name] {
			imports[of.Import] = "."
		}
		if helpers[name] {
			imports[r.project.SetupImport()] = "."
		}
	}
	for _, name := range language.Qualifiers(expression) {
		if r.standardPackage(name) {
			imports[name] = name
		}
	}
}

// importBlock writes imports, each under the name given for it, in a fixed order.
func importBlock(imports map[string]string) string {
	if len(imports) == 0 {
		return ""
	}
	paths := make([]string, 0, len(imports))
	for path := range imports {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	var block strings.Builder
	block.WriteString("import (\n")
	for _, path := range paths {
		fmt.Fprintf(&block, "\t%s %q\n", imports[path], path)
	}
	block.WriteString(")\n")
	return block.String()
}
