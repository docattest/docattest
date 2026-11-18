package analysis

import (
	"go/ast"
	"go/token"
)

// subtestNames are the distinct names a subtest's name argument gives, nil standing for
// any that is computed. A string literal gives itself. In a table-driven test, the key
// variable of an enclosing `for k, v := range table`, or a field `v.f` of its value
// variable, gives each element's map key or field where that is a string literal. The
// table is a composite literal: written in the loop, or a variable of the test function
// assigned once, from one.
func (in testFunction) subtestNames(arg ast.Expr, loops []*ast.RangeStmt) []*string {
	if name := literalName(arg); name != nil {
		return []*string{name}
	}
	names, ok := in.tableNames(arg, loops)
	if !ok {
		return []*string{nil}
	}
	return distinct(names)
}

// tableNames are the names, one per element of a table, that a name argument takes as
// its loop goes over the table; false when it is not a loop variable over a table.
func (in testFunction) tableNames(arg ast.Expr, loops []*ast.RangeStmt) ([]*string, bool) {
	variable, field := loopVariableUse(arg)
	loop := innermostBinding(loops, variable)
	if loop == nil {
		return nil, false
	}
	table := in.table(loop.X)
	switch {
	case table == nil:
		return nil, false
	case field == "" && binds(loop.Key, variable):
		return keyNames(table)
	case field != "" && binds(loop.Value, variable):
		return fieldNames(table, field), true
	}
	return nil, false
}

// loopVariableUse splits a name argument into the variable it reads and the field of it
// it selects, "" for the variable itself.
func loopVariableUse(arg ast.Expr) (variable, field string) {
	switch a := arg.(type) {
	case *ast.Ident:
		return a.Name, ""
	case *ast.SelectorExpr:
		if x, ok := a.X.(*ast.Ident); ok {
			return x.Name, a.Sel.Name
		}
	}
	return "", ""
}

// innermostBinding is the innermost loop that declares a variable, the one a use of its
// name reads.
func innermostBinding(loops []*ast.RangeStmt, variable string) *ast.RangeStmt {
	for k := len(loops) - 1; k >= 0; k-- {
		if binds(loops[k].Key, variable) || binds(loops[k].Value, variable) {
			return loops[k]
		}
	}
	return nil
}

func binds(loopVariable ast.Expr, variable string) bool {
	ident, ok := loopVariable.(*ast.Ident)
	return ok && variable != "" && ident.Name == variable
}

// table is the composite literal a loop ranges over, or nil when it is anything else.
func (in testFunction) table(ranged ast.Expr) *ast.CompositeLit {
	switch r := ranged.(type) {
	case *ast.CompositeLit:
		return r
	case *ast.Ident:
		return in.tables[r.Name]
	}
	return nil
}

// keyNames are a map literal's keys; false when the table is not a map, whose keys are
// indexes.
func keyNames(table *ast.CompositeLit) ([]*string, bool) {
	if _, isMap := table.Type.(*ast.MapType); !isMap {
		return nil, false
	}
	out := []*string{}
	for _, element := range table.Elts {
		keyed, ok := element.(*ast.KeyValueExpr)
		if !ok {
			return nil, false
		}
		out = append(out, literalName(keyed.Key))
	}
	return out, true
}

// fieldNames are the values each element of a table gives a field.
func fieldNames(table *ast.CompositeLit, field string) []*string {
	declared := elementType(table.Type)
	out := []*string{}
	for _, element := range table.Elts {
		if keyed, ok := element.(*ast.KeyValueExpr); ok {
			element = keyed.Value
		}
		if address, ok := element.(*ast.UnaryExpr); ok && address.Op == token.AND {
			element = address.X
		}
		value, ok := element.(*ast.CompositeLit)
		if !ok {
			out = append(out, nil)
			continue
		}
		out = append(out, fieldValue(value, field, declared))
	}
	return out
}

// elementType is the type a slice, array or map literal writes for its elements, or nil.
func elementType(tableType ast.Expr) ast.Expr {
	switch t := tableType.(type) {
	case *ast.ArrayType:
		return t.Elt
	case *ast.MapType:
		return t.Value
	}
	return nil
}

// fieldValue is a field of a struct literal when it is a string literal: by its key, or,
// in a literal without keys, by its position in the struct type the code writes.
func fieldValue(value *ast.CompositeLit, field string, declared ast.Expr) *string {
	if len(value.Elts) > 0 {
		if _, keyed := value.Elts[0].(*ast.KeyValueExpr); keyed {
			return keyedField(value, field)
		}
	}
	if value.Type != nil {
		declared = value.Type
	}
	if pointer, ok := declared.(*ast.StarExpr); ok {
		declared = pointer.X
	}
	structType, ok := declared.(*ast.StructType)
	if !ok {
		return nil
	}
	position := fieldPosition(structType, field)
	if position < 0 || position >= len(value.Elts) {
		return nil
	}
	return literalName(value.Elts[position])
}

func keyedField(value *ast.CompositeLit, field string) *string {
	for _, element := range value.Elts {
		keyed, ok := element.(*ast.KeyValueExpr)
		if !ok {
			return nil
		}
		if key, ok := keyed.Key.(*ast.Ident); ok && key.Name == field {
			return literalName(keyed.Value)
		}
	}
	return nil
}

// fieldPosition is where a field comes in a struct, or -1; an embedded field takes one
// position.
func fieldPosition(structType *ast.StructType, field string) int {
	position := 0
	for _, group := range structType.Fields.List {
		if len(group.Names) == 0 {
			position++
		}
		for _, name := range group.Names {
			if name.Name == field {
				return position
			}
			position++
		}
	}
	return -1
}

// distinct keeps the first of each name: elements with the same name are one subtest's
// code, so they declare it once.
func distinct(names []*string) []*string {
	seen := map[string]bool{}
	sawComputed := false
	out := []*string{}
	for _, name := range names {
		switch {
		case name == nil && !sawComputed:
			sawComputed = true
			out = append(out, nil)
		case name != nil && !seen[*name]:
			seen[*name] = true
			out = append(out, name)
		}
	}
	return out
}

// tablesIn are the variables of a function body assigned once, from a composite literal:
// the tables its table-driven tests range over, by name.
func tablesIn(body *ast.BlockStmt) map[string]*ast.CompositeLit {
	if body == nil {
		return nil
	}
	assignments := map[string]int{}
	literals := map[string]*ast.CompositeLit{}
	assign := func(target ast.Expr, value ast.Expr) {
		ident, ok := target.(*ast.Ident)
		if !ok {
			return
		}
		assignments[ident.Name]++
		if literal, ok := value.(*ast.CompositeLit); ok {
			literals[ident.Name] = literal
		}
	}
	ast.Inspect(body, func(node ast.Node) bool {
		switch n := node.(type) {
		case *ast.AssignStmt:
			for k, target := range n.Lhs {
				assign(target, pairedValue(n.Rhs, k, len(n.Lhs)))
			}
		case *ast.ValueSpec:
			for k, name := range n.Names {
				assign(name, pairedValue(n.Values, k, len(n.Names)))
			}
		}
		return true
	})
	tables := map[string]*ast.CompositeLit{}
	for name, literal := range literals {
		if assignments[name] == 1 {
			tables[name] = literal
		}
	}
	return tables
}

// pairedValue is the value assigned to the k-th of n targets, or nil when one call
// returns them all.
func pairedValue(values []ast.Expr, k, n int) ast.Expr {
	if len(values) != n {
		return nil
	}
	return values[k]
}
