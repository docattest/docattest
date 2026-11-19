package running

import (
	"fmt"
	"go/constant"
	"go/types"
	"math/big"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/analysis"
)

// A constant that no program can import, any constant of a package main and an
// unexported one of any other package, is read from source: the analysis type-checks
// the surface's package, and the generated program declares the constant with the value
// the type checker gives it. Nothing imports the package and nothing of it runs.

// sourceConstant is a constant read from source, as a generated program declares it.
type sourceConstant struct {
	name string
	// typeName is the type the declaration names: "" for an untyped constant, a basic
	// type's name, or a type of the package at typePath.
	typeName, typePath string
	literal            string
}

// readConstants gives those of `names` that are constants a surface's package declares.
func (r *Runner) readConstants(of *analysis.Surface, names []string) (map[string]sourceConstant, error) {
	pkg, err := r.project.Checked(of)
	if err != nil {
		return nil, err
	}
	out := map[string]sourceConstant{}
	for _, name := range names {
		if found, isConstant := pkg.Scope().Lookup(name).(*types.Const); isConstant {
			out[name] = declared(found, r.programPath())
		}
	}
	return out, nil
}

// programPath is the import path of the generated programs: what decides which types
// they can name.
func (r *Runner) programPath() string {
	return r.project.Module.Path + "/" + blocksDir
}

// declared is how a generated program declares a constant. Its type is named when a
// program of the module can import it. Otherwise the declaration names the type's
// underlying type: a constant whose type a package main or an unexported name declares
// is shown as its underlying value, without that type's String method.
func declared(found *types.Const, program string) sourceConstant {
	out := sourceConstant{name: found.Name(), literal: literal(found.Val(), found.Type())}
	switch typed := types.Unalias(found.Type()).(type) {
	case *types.Basic:
		if typed.Info()&types.IsUntyped == 0 {
			out.typeName = typed.Name()
		}
	case *types.Named:
		out.typeName = typed.Underlying().String()
		if nameable(typed, program) {
			out.typeName, out.typePath = typed.Obj().Name(), typed.Obj().Pkg().Path()
		}
	}
	return out
}

// nameable says whether a program can name a type: an exported type, not an instance of
// a generic one, of a package other than a main that the program may import.
func nameable(named *types.Named, program string) bool {
	declaredBy := named.Obj().Pkg()
	return named.Obj().Exported() && named.TypeArgs().Len() == 0 && declaredBy.Name() != "main" &&
		visible(declaredBy.Path(), program)
}

// visible applies Go's rule for internal packages: a package inside a directory named
// internal is imported only by the tree rooted at that directory's parent.
func visible(path, program string) bool {
	padded := "/" + path + "/"
	at := strings.LastIndex(padded, "/internal/")
	if at < 0 {
		return true
	}
	parent := strings.TrimPrefix(padded[:at], "/")
	return parent != "" && strings.HasPrefix(program+"/", parent+"/")
}

// literal writes a constant's value as Go source of the same kind: an untyped constant
// stays the kind of untyped constant it is, so that `1.0 / 3` divides as a float.
func literal(value constant.Value, of types.Type) string {
	switch {
	case value.Kind() == constant.Float:
		return floatLiteral(value)
	case value.Kind() == constant.Complex:
		realPart, imaginaryPart := constant.ToFloat(constant.Real(value)), constant.ToFloat(constant.Imag(value))
		return fmt.Sprintf("complex(%s, %s)", floatLiteral(realPart), floatLiteral(imaginaryPart))
	case types.Identical(of, types.Typ[types.UntypedRune]):
		// A rune plus an integer is a rune, whatever its value.
		return fmt.Sprintf("('\\x00' + %s)", value.ExactString())
	}
	return value.ExactString()
}

// floatLiteral writes the value of a float constant exactly, as a float.
func floatLiteral(value constant.Value) string {
	fraction, small := constant.Val(value).(*big.Rat)
	switch {
	case !small:
		// Only a value too large or too small for a fraction is a *big.Float, and 'g'
		// writes it with an exponent, as a float.
		return constant.Val(value).(*big.Float).Text('g', -1)
	case fraction.IsInt():
		return fraction.Num().String() + ".0"
	}
	return fmt.Sprintf("(%s.0 / %s)", fraction.Num(), fraction.Denom())
}

// declaration is the Go statement that declares a constant, with its type's package
// under the name `imports` gives it.
func (c sourceConstant) declaration(imports *programImports) string {
	switch {
	case c.typeName == "":
		return fmt.Sprintf("const %s = %s", c.name, c.literal)
	case c.typePath == "":
		return fmt.Sprintf("const %s %s = %s", c.name, c.typeName, c.literal)
	}
	return fmt.Sprintf("const %s %s.%s = %s", c.name, imports.alias(c.typePath), c.typeName, c.literal)
}
