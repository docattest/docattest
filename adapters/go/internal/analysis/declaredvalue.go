package analysis

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"go/types"
	"os"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// declaredValue is the value a constant or a variable is declared with, as its file
// writes it, which `mutants` changes.
//
// A change is offered only when the value still type-checks with it, as the value of its
// declaration: in its file's scope, converted to the type it is declared with. That is
// what keeps a typed constant within its type (`const Max uint8 = 255` is not made 256)
// and a map's keys apart. Only the value is checked: a change that breaks a use of it
// elsewhere, such as two cases of a switch that become the same, is not seen.
type declaredValue struct {
	expr ast.Expr
	pkg  *goPackage
	file *token.File
	// path is the file's project path.
	path string
	text []byte
	// convertedTo is the type the value is converted to where it is declared, as Go writes
	// it: the declaration's own, or the default type of a variable's untyped constant; ""
	// for a constant declared without a type.
	convertedTo string
	// checkable says whether a change can be type-checked: the value as written checks,
	// and declares no function type. Checking one would add the function's scope to the
	// file's, which the package's type information shares with every other request.
	checkable bool
}

// readDeclaredValue reads the value a constant or a variable of a package is declared
// with from its file.
func (p *Project) readDeclaredValue(pkg *goPackage, object types.Object, expr ast.Expr) (declaredValue, error) {
	file := pkg.Fset.File(expr.Pos())
	text, err := os.ReadFile(file.Name())
	if err != nil {
		return declaredValue{}, fmt.Errorf("cannot read %s: %w", file.Name(), err)
	}
	value := declaredValue{expr: expr, pkg: pkg, file: file, path: p.rel(file.Name()), text: text}
	value.convertedTo = value.declaredType(pkg.declaredBy[object].(*ast.ValueSpec), object)
	as, parsed := value.parse(value.source(expr.Pos(), expr.End()))
	value.checkable = parsed && !declaresFunctionType(as) && value.checks(as)
	return value, nil
}

// declaredType is the type a spec converts a value to, as Go writes it; "" for none.
func (d declaredValue) declaredType(spec *ast.ValueSpec, object types.Object) string {
	if spec.Type != nil {
		return d.source(spec.Type.Pos(), spec.Type.End())
	}
	if _, isVariable := object.(*types.Var); isVariable {
		if basic, isBasic := object.Type().(*types.Basic); isBasic {
			return basic.Name()
		}
	}
	return ""
}

// edit is a change as an edit of the value's file.
func (d declaredValue) edit(change literalMutant) protocol.MutantEdit {
	return protocol.MutantEdit{
		Path: d.path, Start: d.file.Offset(change.start), End: d.file.Offset(change.end),
		Replacement: change.replacement, Description: change.description,
	}
}

// compiles says whether the value still type-checks with a change made to it; true when
// that cannot be checked.
func (d declaredValue) compiles(change literalMutant) bool {
	if !d.checkable {
		return true
	}
	changed := d.source(d.expr.Pos(), change.start) + change.replacement + d.source(change.end, d.expr.End())
	as, parsed := d.parse(changed)
	return parsed && d.checks(as)
}

// source is the text of the value's file from one position to another.
func (d declaredValue) source(from, to token.Pos) string {
	return string(d.text[d.file.Offset(from):d.file.Offset(to)])
}

// parse parses the text of a value, converted to the type it is declared with.
func (d declaredValue) parse(value string) (ast.Expr, bool) {
	if d.convertedTo != "" {
		value = "(" + d.convertedTo + ")(" + value + ")"
	}
	expr, err := parser.ParseExpr(value)
	return expr, err == nil
}

// checks says whether a value type-checks in the scope of the value's file.
func (d declaredValue) checks(expr ast.Expr) bool {
	return types.CheckExpr(d.pkg.Fset, d.pkg.Types, d.expr.Pos(), expr, nil) == nil
}

// declaresFunctionType says whether an expression writes a function type: a function
// literal's, or one in a type it names.
func declaresFunctionType(expr ast.Expr) bool {
	found := false
	ast.Inspect(expr, func(n ast.Node) bool {
		_, isFunction := n.(*ast.FuncType)
		found = found || isFunction
		return !found
	})
	return found
}
