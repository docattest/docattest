package running

import (
	"fmt"
	"slices"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// What a fact reads from source, of the names a program importing its surface cannot
// use, are its constants (constants.go) and its literal variables (variables.go), each
// declared in the fact's own function.

// sourceValues are what a surface's facts read from source, by name.
type sourceValues struct {
	constants map[string]sourceConstant
	variables map[string]sourceVariable
}

// prelude is what a fact's function starts with: the declarations of what it reads from
// source, the types its variables copy first, each once.
func (values sourceValues) prelude(names []string, imports *programImports) string {
	copied, declarations := []string{}, ""
	for _, name := range names {
		if read, found := values.constants[name]; found {
			declarations += read.declaration(imports) + "; "
		}
		read, found := values.variables[name]
		if !found {
			continue
		}
		for _, declaration := range read.copied {
			if text := declaration.in(imports); !slices.Contains(copied, text) {
				copied = append(copied, text)
			}
		}
		declarations += read.statements.in(imports) + "; "
	}
	prelude := ""
	for _, declaration := range copied {
		prelude += declaration + "; "
	}
	return prelude + declarations
}

// sourceName is a name of a surface that a program importing it cannot use.
type sourceName struct {
	name string
	what analysis.Declares
}

// sourceNames are the names an expression uses that a program importing its surface
// cannot use, each once, in order. A name that something is selected from and that names
// a package of the standard library is that package, as it is in every other fact.
func (r *Runner) sourceNames(of *analysis.Surface, expression string) []sourceName {
	sourceOnly := r.project.SourceOnly(of)
	skipped := map[string]bool{}
	for _, name := range language.Qualifiers(expression) {
		skipped[name] = r.standardPackage(name)
	}
	out := []sourceName{}
	for _, name := range language.Names(expression) {
		if what, found := sourceOnly[name]; found && !skipped[name] {
			out, skipped[name] = append(out, sourceName{name, what}), true
		}
	}
	return out
}

// sourceRead are the names of the constants and the variables an expression reads from
// source.
func (r *Runner) sourceRead(of *analysis.Surface, expression string) []string {
	names := []string{}
	for _, used := range r.sourceNames(of, expression) {
		if used.what == analysis.Constant || used.what == analysis.Variable {
			names = append(names, used.name)
		}
	}
	return names
}

// withSourceValues reads what a surface's facts read from source. It returns the facts
// that can go on, what they read, and the evaluations of the facts that cannot: all that
// read from source when the package cannot be type-checked, and each that reads a
// variable that is not a literal variable or whose type a fact cannot copy.
func (r *Runner) withSourceValues(of *analysis.Surface, facts []protocol.Fact) ([]protocol.Fact, sourceValues, map[string]protocol.Evaluation) {
	constants, variables := []string{}, []string{}
	for _, fact := range facts {
		for _, used := range r.sourceNames(of, fact.Expression) {
			if used.what == analysis.Constant {
				constants = append(constants, used.name)
			} else {
				variables = append(variables, used.name)
			}
		}
	}
	values := sourceValues{variables: map[string]sourceVariable{}}
	if len(constants)+len(variables) == 0 {
		return facts, values, map[string]protocol.Evaluation{}
	}
	var err error
	if values.constants, err = r.readConstants(of, constants); err != nil {
		kept, failed := keepFacts(facts, func(fact protocol.Fact) string {
			if len(r.sourceRead(of, fact.Expression)) > 0 {
				return fmt.Sprintf("surface %q failed to load: %v", of.Name, err)
			}
			return ""
		})
		return kept, values, failed
	}
	unread := map[string]error{}
	for _, name := range variables {
		if values.variables[name], err = r.readSourceVariable(of, name); err != nil {
			unread[name] = err
		}
	}
	kept, failed := keepFacts(facts, func(fact protocol.Fact) string {
		for _, name := range r.sourceRead(of, fact.Expression) {
			if unread[name] != nil {
				return fmt.Sprintf("fact `%s`: %v", fact.Expression, unread[name])
			}
		}
		return ""
	})
	return kept, values, failed
}

// readSourceVariable reads a variable of a surface's package from source; an error says
// why a fact cannot.
func (r *Runner) readSourceVariable(of *analysis.Surface, name string) (sourceVariable, error) {
	read, err := r.project.LiteralVariable(of, name)
	if err == nil {
		var variable sourceVariable
		if variable, err = r.readVariable(read); err == nil {
			return variable, nil
		}
	}
	return sourceVariable{}, fmt.Errorf("%s is a variable of surface %q that a fact cannot read from source: %w", name, of.Name, err)
}

// keepFacts are the facts that `why` says nothing of, and the failed evaluations of the
// others, each with what it says.
func keepFacts(facts []protocol.Fact, why func(protocol.Fact) string) ([]protocol.Fact, map[string]protocol.Evaluation) {
	kept, failed := []protocol.Fact{}, map[string]protocol.Evaluation{}
	for _, fact := range facts {
		if message := why(fact); message != "" {
			failed[fact.ID] = protocol.Failed(message)
		} else {
			kept = append(kept, fact)
		}
	}
	return kept, failed
}
