package analysis

import (
	"go/ast"
	"go/types"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// A fact shows the values it reads, and a cases runner checks its rows against the values
// it reads, so what either attests of a constant or a variable of a surface is its value.
// Such a proof attests each value it reaches, by name, through a helper or in its runner,
// when the value is an export, or a constant or a literal variable, which a fact reads from
// source exported or not. A value holds the values its initializer names, so the proof
// also attests what that initializer names, at any depth: a constant built from constants,
// a literal variable whose elements name constants, a variable built from another. A
// function an initializer calls is attested and its code is not followed: what it returns
// is what its own proofs attest. An example or a test attests the exports it names.

// attesting is what a proof attests of the names its code reaches.
type attesting int

const (
	// namesUsed is an example's or a test's: the exports it names.
	namesUsed attesting = iota
	// valuesRead is a fact's or a cases runner's: the values it reads, as above.
	valuesRead
)

// attestingOf is what a kind of attestation attests of the names its code reaches.
func attestingOf(kind protocol.Kind) attesting {
	if kind == protocol.KindFact || kind == protocol.KindCases {
		return valuesRead
	}
	return namesUsed
}

// readName records a top-level name of a surface's package that a fact or a cases runner
// reads, when it attests it: an export that a program importing the surface can use, or a
// constant or a literal variable, read from source. Of a constant or a variable, it then
// follows the initializer.
func (r *refs) readName(name string, of *Surface) {
	pkg := r.project.tree().at(of.Dir)
	object := pkg.Types.Scope().Lookup(name)
	spec, isValue := pkg.declaredBy[object].(*ast.ValueSpec)
	switch {
	case !pkg.isSourceOnly(name):
		r.add(name, of)
	case isValue && pkg.readFromSource(declared{Name: name, What: valueKind(object)}):
		r.record(name, of)
	}
	if isValue {
		r.followInitializer(object, spec, of)
	}
}

// followInitializer collects what the initializer of a value of a surface's package
// names, once for each value.
func (r *refs) followInitializer(object types.Object, spec *ast.ValueSpec, of *Surface) {
	ref := protocol.SymbolRef{Name: object.Name(), Surface: of.Name}
	if r.followed[ref] {
		return
	}
	r.followed[ref] = true
	r.collect(initializer(spec, object), r.project.tree().at(of.Dir).Info)
}

// valueKind is what a top-level constant or variable declares.
func valueKind(object types.Object) Declares {
	if _, isConstant := object.(*types.Const); isConstant {
		return Constant
	}
	return Variable
}
