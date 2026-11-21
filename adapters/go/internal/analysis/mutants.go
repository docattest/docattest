package analysis

import (
	"fmt"
	"go/ast"
	"go/types"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// Mutants answers `mutants`: for each symbol, in order, at most `limit` edits to the
// literals of its declaration's value (literalmutants.go says which), the first in source
// order, each one that leaves the value valid Go (declaredvalue.go), or why there are
// none. A symbol with a surface is a name its surface's package declares at its top
// level. One without is what `symbols` would give without one: a constant or a literal
// variable that a fact reads from source, of the first surface whose package declares it.
func (p *Project) Mutants(symbols []protocol.SymbolRef, limit int) ([]protocol.SymbolMutants, error) {
	if p.treeFailed != nil {
		return nil, fmt.Errorf("cannot load the code of the surfaces: %w", p.treeFailed)
	}
	out := make([]protocol.SymbolMutants, len(symbols))
	for k, symbol := range symbols {
		var err error
		if out[k], err = p.mutantsOf(symbol, limit); err != nil {
			return nil, err
		}
	}
	return out, nil
}

// mutantsOf are the edits of one symbol, or why it has none. A function, a method or a
// type has none, and neither has a value declared without a value or whose value holds
// no literal that can be changed.
func (p *Project) mutantsOf(symbol protocol.SymbolRef, limit int) (protocol.SymbolMutants, error) {
	pkg, object := p.declaring(symbol)
	switch {
	case object == nil || !within(p.Root, pkg.Fset.File(object.Pos()).Name()):
		return noMutants(protocol.WhyNotFound), nil
	case !isValueObject(object):
		return noMutants(protocol.WhyFunction), nil
	}
	expr := initializer(pkg.declaredBy[object].(*ast.ValueSpec), object)
	if expr == nil {
		return noMutants(protocol.WhyNoLiteral), nil
	}
	value, err := p.readDeclaredValue(pkg, object, expr)
	if err != nil {
		return protocol.SymbolMutants{}, err
	}
	edits := []protocol.MutantEdit{}
	candidates := literalMutants(expr, pkg.Info)
	if len(candidates) == 0 {
		for _, change := range expressionMutants(expr, object, pkg.Info, value.source(expr.Pos(), expr.End())) {
			if value.compiles(change) {
				edits = append(edits, value.edit(change))
				break
			}
		}
	} else {
		for _, change := range candidates {
			if len(edits) == limit {
				break
			}
			if value.compiles(change) {
				edits = append(edits, value.edit(change))
			}
		}
	}
	if len(edits) == 0 {
		return noMutants(protocol.WhyNoLiteral), nil
	}
	return protocol.SymbolMutants{Edits: edits}, nil
}

func noMutants(why protocol.MutantsWhy) protocol.SymbolMutants {
	return protocol.SymbolMutants{Edits: []protocol.MutantEdit{}, Why: why}
}

// declaring is the package that declares a symbol and what it declares; a nil object when
// nothing declares it.
func (p *Project) declaring(symbol protocol.SymbolRef) (*goPackage, types.Object) {
	if symbol.Surface != "" {
		return p.declaredIn(p.SurfaceNamed(symbol.Surface), symbol.Name)
	}
	for _, of := range p.surfaces {
		pkg, object := p.declaredIn(of, symbol.Name)
		if object != nil && isValueObject(object) && pkg.readFromSource(declared{Name: symbol.Name, What: valueKind(object)}) {
			return pkg, object
		}
	}
	return nil, nil
}

// declaredIn is a surface's package and what it declares of a name at its top level; a
// nil object when the surface did not resolve or does not declare the name.
func (p *Project) declaredIn(of *Surface, name string) (*goPackage, types.Object) {
	if of == nil || of.Dir == "" {
		return nil, nil
	}
	pkg := p.tree().at(of.Dir)
	if pkg == nil {
		return nil, nil
	}
	return pkg, pkg.Types.Scope().Lookup(name)
}

// isValueObject says whether an object is a constant or a variable.
func isValueObject(object types.Object) bool {
	switch object.(type) {
	case *types.Const, *types.Var:
		return true
	}
	return false
}
