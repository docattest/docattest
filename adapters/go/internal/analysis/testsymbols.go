package analysis

import (
	"go/ast"
	"go/types"
)

// collectCitation collects the surface symbols used by a cited test and by the local test
// helpers and enclosing table/variable initializers it references, transitively, without
// crossing into production functions or sibling subtests.
func (r *refs) collectCitation(test *declaredTest, code typedFile) {
	if test == nil || code.info == nil {
		return
	}
	decls := testDeclarations(code.testFiles, code.info)
	if test.Enclosing != nil && test.Node != test.Enclosing {
		addLocalDecls(decls, test.Enclosing, code.info)
	}
	seen := map[ast.Node]bool{test.Node: true}
	queue := []ast.Node{test.Node}
	for k := 0; k < len(queue); k++ {
		node := queue[k]
		r.collect(node, code.info)
		ast.Inspect(node, func(n ast.Node) bool {
			ident, ok := n.(*ast.Ident)
			if !ok {
				return true
			}
			obj := origin(code.info.Uses[ident])
			if target := decls[obj]; target != nil && !seen[target] {
				seen[target] = true
				queue = append(queue, target)
			}
			return true
		})
	}
}

// testDeclarations maps every top-level function, method, variable and constant declared
// in `_test.go` files of the test package to its AST declaration.
func testDeclarations(files []*ast.File, info *types.Info) map[types.Object]ast.Node {
	out := map[types.Object]ast.Node{}
	for _, file := range files {
		for _, decl := range file.Decls {
			switch d := decl.(type) {
			case *ast.FuncDecl:
				if obj := info.Defs[d.Name]; obj != nil {
					out[obj] = d
				}
			case *ast.GenDecl:
				recordValueSpecs(out, d, info)
			}
		}
	}
	return out
}

// addLocalDecls maps local variables, constants and range-loop variables in `enclosing`
// to their initializer expressions (or range target), skipping `t.Run` subtest bodies so
// sibling subtests never leak into each other.
func addLocalDecls(out map[types.Object]ast.Node, enclosing *ast.FuncDecl, info *types.Info) {
	if enclosing.Body == nil {
		return
	}
	ast.Inspect(enclosing.Body, func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.CallExpr:
			if _, isRun := subtestBody(x); isRun {
				return false
			}
		case *ast.AssignStmt:
			recordAssignDefs(out, x, info)
		case *ast.RangeStmt:
			recordRangeDefs(out, x, info)
		case *ast.GenDecl:
			recordValueSpecs(out, x, info)
		}
		return true
	})
}

func recordValueSpecs(out map[types.Object]ast.Node, decl *ast.GenDecl, info *types.Info) {
	for _, spec := range decl.Specs {
		vs, ok := spec.(*ast.ValueSpec)
		if !ok {
			continue
		}
		for k, name := range vs.Names {
			if obj := info.Defs[name]; obj != nil && k < len(vs.Values) {
				out[obj] = vs.Values[k]
			}
		}
	}
}

func recordAssignDefs(out map[types.Object]ast.Node, stmt *ast.AssignStmt, info *types.Info) {
	for k, lhs := range stmt.Lhs {
		ident, ok := lhs.(*ast.Ident)
		if !ok {
			continue
		}
		obj := info.Defs[ident]
		if obj == nil {
			continue
		}
		if k < len(stmt.Rhs) {
			out[obj] = stmt.Rhs[k]
		} else if len(stmt.Rhs) == 1 {
			out[obj] = stmt.Rhs[0]
		}
	}
}

func recordRangeDefs(out map[types.Object]ast.Node, stmt *ast.RangeStmt, info *types.Info) {
	for _, lhs := range []ast.Expr{stmt.Key, stmt.Value} {
		if ident, ok := lhs.(*ast.Ident); ok {
			if obj := info.Defs[ident]; obj != nil {
				out[obj] = stmt.X
			}
		}
	}
}
