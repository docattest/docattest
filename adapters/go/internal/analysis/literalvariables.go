package analysis

import (
	"errors"
	"fmt"
	"go/ast"
	"go/types"
)

// A fact reads a variable of its surface's package from source, as it reads a constant,
// when the variable is a literal variable: one that its declaration at the top level
// gives a composite literal whose elements (and keys) are constants or such literals,
// and that the package only reads (variablechanges.go). Its value is then the one the
// source writes whenever the program reads it, and the type checker knows each of its
// constants: nothing of the package has to run.

// LiteralVariable is a literal variable of a surface's package, as the type checker
// typed it.
type LiteralVariable struct {
	Object *types.Var
	// Value is the composite literal its declaration gives it.
	Value ast.Expr
	// Info is what the type checker found of the names and expressions of its package.
	Info *types.Info
}

// LiteralVariable is a variable of a surface's package that a fact reads from source; an
// error says why it cannot be read so. The surface resolved, its package compiles, and it
// declares the variable at its top level.
func (p *Project) LiteralVariable(of *Surface, name string) (LiteralVariable, error) {
	return p.tree().at(of.Dir).literalVariable(name)
}

// readFromSource says whether a fact reads a name a package declares from source: a
// constant, or a literal variable.
func (pkg *goPackage) readFromSource(d declared) bool {
	if d.What != Variable {
		return d.What == Constant
	}
	_, err := pkg.literalVariable(d.Name)
	return err == nil
}

// literalVariable is a variable the package declares at its top level, when it is a
// literal variable.
func (pkg *goPackage) literalVariable(name string) (LiteralVariable, error) {
	object := pkg.Types.Scope().Lookup(name).(*types.Var)
	value := initializer(pkg.declaredBy[object].(*ast.ValueSpec), object)
	if value == nil {
		return LiteralVariable{}, errors.New("it is declared without a value")
	}
	if _, isLiteral := ast.Unparen(value).(*ast.CompositeLit); !isLiteral {
		return LiteralVariable{}, fmt.Errorf("its value, %s, is not a composite literal", types.ExprString(value))
	}
	if part := pkg.notConstant(value); part != nil {
		return LiteralVariable{}, fmt.Errorf("%s (%s)", notConstantWhy(part), pkg.position(part))
	}
	if change := pkg.changes()[object]; change != "" {
		return LiteralVariable{}, fmt.Errorf("the package may change it (%s)", change)
	}
	return LiteralVariable{Object: object, Value: value, Info: pkg.Info}, nil
}

// initializer is the expression a spec gives one of the values it declares: its own, or
// the one call that gives them all; nil when it gives none.
func initializer(spec *ast.ValueSpec, object types.Object) ast.Expr {
	switch {
	case len(spec.Values) == 1:
		return spec.Values[0]
	case len(spec.Values) == 0:
		return nil
	}
	for k, name := range spec.Names {
		if name.Name == object.Name() {
			return spec.Values[k]
		}
	}
	return nil
}

// notConstant is the first part of a variable's value that is neither a constant nor a
// composite literal of such parts; nil when there is none. A struct literal's keys are
// its fields' names. A literal whose `&` is left out (an item of a `[]*T`) is a pointer,
// and not a composite literal of a value.
func (pkg *goPackage) notConstant(value ast.Expr) ast.Expr {
	if pkg.Info.Types[value].Value != nil {
		return nil
	}
	switch v := value.(type) {
	case *ast.ParenExpr:
		return pkg.notConstant(v.X)
	case *ast.CompositeLit:
		if _, isPointer := pkg.Info.TypeOf(v).Underlying().(*types.Pointer); isPointer {
			return v
		}
		_, isStruct := pkg.Info.TypeOf(v).Underlying().(*types.Struct)
		for _, element := range v.Elts {
			if bad := pkg.notConstantElement(element, isStruct); bad != nil {
				return bad
			}
		}
		return nil
	}
	return value
}

// notConstantWhy says why a part that notConstant gives keeps a variable from being a
// literal variable.
func notConstantWhy(part ast.Expr) string {
	if _, isLiteral := part.(*ast.CompositeLit); isLiteral {
		return "its value holds a pointer"
	}
	return types.ExprString(part) + " in its value is not a constant"
}

// notConstantElement is notConstant of an element of a composite literal, with its key.
func (pkg *goPackage) notConstantElement(element ast.Expr, ofStruct bool) ast.Expr {
	pair, keyed := element.(*ast.KeyValueExpr)
	if !keyed {
		return pkg.notConstant(element)
	}
	if !ofStruct {
		if bad := pkg.notConstant(pair.Key); bad != nil {
			return bad
		}
	}
	return pkg.notConstant(pair.Value)
}
