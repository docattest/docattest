package analysis

import (
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"path/filepath"
	"slices"
)

// A package only reads a variable when no use of it in the package's files (its docs
// helper files included, its tests not, since they are no part of a program) may change
// it. A use is the variable followed by any index, slice and field selections, and it
// may change the variable when it
//
//   - is assigned to, incremented or decremented, or ranged into;
//   - has its address taken, also by calling a method with a pointer receiver;
//   - is anything else that shares the variable's storage, a slice, a map, a pointer or
//     an interface, unless it is given to len or cap, copied from (copy's source, or
//     append's `s...`), or ranged over with items that share nothing.
//
// A value that shares nothing (a basic value, or an array or struct of such) is copied
// wherever it goes, so it may go anywhere. The rule is syntactic and sound: a use that
// may change the variable makes it no literal variable, even when the code it is given
// to only reads it.

// findChanges are the package-level variables that a use in the package may change, each
// with the first such use: where it is and what it does.
func (pkg *goPackage) findChanges() map[types.Object]string {
	out := map[types.Object]string{}
	for _, file := range sorted(pkg.Files) {
		stack := []ast.Node{}
		ast.Inspect(file, func(n ast.Node) bool {
			if n == nil {
				stack = stack[:len(stack)-1]
				return true
			}
			stack = append(stack, n)
			if name, isName := n.(*ast.Ident); isName {
				pkg.noteChange(out, name, stack)
			}
			return true
		})
	}
	return out
}

// noteChange notes how the use of a package-level variable at the top of a stack, the
// innermost node last, may change it, when it may and nothing earlier did.
func (pkg *goPackage) noteChange(out map[types.Object]string, name *ast.Ident, stack []ast.Node) {
	object, isVar := pkg.Info.Uses[name].(*types.Var)
	if !isVar || object.Parent() != pkg.Types.Scope() || out[object] != "" {
		return
	}
	if what := mayChange(stack, pkg.Info); what != "" {
		out[object] = fmt.Sprintf("%s %s", pkg.position(name), what)
	}
}

// position is where a node of the package is: its file's name and its line.
func (pkg *goPackage) position(node ast.Node) string {
	at := pkg.Fset.Position(node.Pos())
	return fmt.Sprintf("%s:%d", filepath.Base(at.Filename), at.Line)
}

// mayChange says how a use of a variable, at the top of a stack, may change it; "" when
// it only reads it.
func mayChange(stack []ast.Node, info *types.Info) string {
	k := len(stack) - 1
	for ; k > 0; k-- {
		selected, through := selectedFrom(stack[k-1], stack[k], info)
		if !through {
			break
		}
		if selected != "" {
			return selected
		}
	}
	use := stack[k].(ast.Expr)
	return usedBy(stack[k-1], use, info)
}

// selectedFrom says whether a parent node selects from a use (an index, a slice, a field,
// or parentheses), so that the use goes on as the parent. A method selected on the use
// ends it: how the method's receiver may change it is then said.
func selectedFrom(parent, use ast.Node, info *types.Info) (change string, through bool) {
	switch p := parent.(type) {
	case *ast.ParenExpr:
		return "", true
	case *ast.IndexExpr:
		return "", p.X == use
	case *ast.SliceExpr:
		return "", p.X == use
	case *ast.SelectorExpr:
		selection := info.Selections[p]
		if selection == nil || p.X != use {
			return "", false
		}
		if selection.Kind() == types.FieldVal {
			return "", true
		}
		return receiverChange(selection, info.TypeOf(p.X)), true
	}
	return "", false
}

// receiverChange says how calling a method may change what it is called on.
func receiverChange(method *types.Selection, on types.Type) string {
	_, byPointer := method.Obj().Type().(*types.Signature).Recv().Type().(*types.Pointer)
	_, onPointer := on.Underlying().(*types.Pointer)
	switch {
	case byPointer && !onPointer:
		return "takes its address to call a method"
	case !sharesNothing(on):
		return "calls a method on it"
	}
	return ""
}

// usedBy says how a node may change a use of a variable it holds.
func usedBy(parent ast.Node, use ast.Expr, info *types.Info) string {
	switch p := parent.(type) {
	case *ast.AssignStmt:
		if slices.Contains(p.Lhs, use) {
			return "assigns to it"
		}
	case *ast.IncDecStmt:
		return "assigns to it"
	case *ast.UnaryExpr:
		if p.Op == token.AND {
			return "takes its address"
		}
	case *ast.RangeStmt:
		return rangeChange(p, use, info)
	}
	if sharesNothing(info.TypeOf(use)) || onlyCopiedFrom(parent, use, info) {
		return ""
	}
	return "lets its storage be shared"
}

// rangeChange says how a range statement may change a use of a variable it holds: as
// what it ranges into, or through items that share the variable's storage.
func rangeChange(loop *ast.RangeStmt, use ast.Expr, info *types.Info) string {
	if loop.Key == use || loop.Value == use {
		return "assigns to it"
	}
	if item, named := loop.Value.(*ast.Ident); loop.Value == nil || (named && item.Name == "_") {
		return ""
	}
	if !sharesNothing(info.TypeOf(loop.Value)) {
		return "ranges over it with items that share its storage"
	}
	return ""
}

// onlyCopiedFrom says whether a call only reads a slice or a map it is given: len or cap
// of it, copy from it, or append its items.
func onlyCopiedFrom(parent ast.Node, use ast.Expr, info *types.Info) bool {
	call, isCall := parent.(*ast.CallExpr)
	if !isCall {
		return false
	}
	name, isName := ast.Unparen(call.Fun).(*ast.Ident)
	if !isName {
		return false
	}
	if _, isBuiltin := info.Uses[name].(*types.Builtin); !isBuiltin {
		return false
	}
	switch name.Name {
	case "len", "cap":
		return true
	case "copy":
		return call.Args[1] == use
	case "append":
		return call.Ellipsis.IsValid() && len(call.Args) > 1 && call.Args[len(call.Args)-1] == use
	}
	return false
}

// sharesNothing says whether a value of a type is all copied when it is: a basic value,
// or an array or a struct of such. A map's `v, ok := m[k]` has the type of both values.
func sharesNothing(of types.Type) bool {
	switch t := of.Underlying().(type) {
	case *types.Basic:
		return t.Kind() != types.UnsafePointer
	case *types.Array:
		return sharesNothing(t.Elem())
	case *types.Tuple:
		for k := range t.Len() {
			if !sharesNothing(t.At(k).Type()) {
				return false
			}
		}
		return true
	case *types.Struct:
		for k := range t.NumFields() {
			if !sharesNothing(t.Field(k).Type()) {
				return false
			}
		}
		return true
	}
	return false
}
