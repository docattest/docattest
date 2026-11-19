package analysis

import (
	"go/ast"
	"go/constant"
	"go/token"
	"go/types"
	"strconv"
	"strings"
)

// The changes `mutants` offers of a value are to its literals, in source order, at any
// depth, except inside a function literal the value defines, which runs only when called,
// and inside a type:
//
//   - an integer becomes that number plus one, written in decimal (`0x10 → 17`);
//   - a floating-point number becomes that number plus one, written with a `.` or an
//     exponent so that Go still reads a float (`1e3 → 1001.0`);
//   - a string, interpreted or raw, gains an `x` before its closing quote, its escapes kept;
//   - the predeclared `true` and `false` are flipped; a name that shadows them is no
//     boolean literal;
//   - a slice literal, or an array literal whose length is `...`, loses its last item,
//     before the changes of its items; an array whose length is written keeps its items;
//   - a map literal's keys are changed as its values are; a struct literal's keys are
//     field names, and a slice's or an array's are indices, which are left as they are.
//
// A rune, an imaginary number, `nil` and `iota` are left as they are.

// literalMutant is one change to a value: the code from start to end is replaced.
type literalMutant struct {
	start, end  token.Pos
	replacement string
	// description says what changes, as Go writes it: `8080 → 8081`.
	description string
}

// literalMutants are the changes to the literals of a value, in source order.
func literalMutants(value ast.Expr, info *types.Info) []literalMutant {
	walk := &mutantWalk{info: info}
	walk.expression(value)
	return walk.found
}

// mutantWalk collects the changes of a value's literals as it walks the value.
type mutantWalk struct {
	info  *types.Info
	found []literalMutant
}

// expression collects the changes of the literals of an expression, in source order.
func (w *mutantWalk) expression(expr ast.Expr) {
	ast.Inspect(expr, func(n ast.Node) bool {
		if part, isExpr := n.(ast.Expr); isExpr && w.info.Types[part].IsType() {
			return false
		}
		switch x := n.(type) {
		case *ast.FuncLit:
			return false
		case *ast.CompositeLit:
			w.composite(x)
			return false
		case *ast.BasicLit:
			w.basic(x)
		case *ast.Ident:
			w.boolean(x)
		}
		return true
	})
}

// composite collects a composite literal's changes: its last item removed, of a list,
// then the changes of its items, with the keys of a map.
func (w *mutantWalk) composite(literal *ast.CompositeLit) {
	shape := shapeOf(literal, w.info)
	if shape == listLiteral && len(literal.Elts) > 0 {
		w.found = append(w.found, lastRemoved(literal))
	}
	for _, element := range literal.Elts {
		pair, keyed := element.(*ast.KeyValueExpr)
		switch {
		case !keyed:
			w.expression(element)
		case shape == mapLiteral:
			w.expression(pair.Key)
			w.expression(pair.Value)
		default:
			w.expression(pair.Value)
		}
	}
}

// literalShape is what a composite literal is, as far as its changes go.
type literalShape int

const (
	// otherLiteral is a struct, or an array whose length is written.
	otherLiteral literalShape = iota
	// listLiteral is a slice, or an array whose length is `...`: it may lose an item.
	listLiteral
	mapLiteral
)

// shapeOf is what a composite literal is. One whose `&` is left out (an item of a
// `[]*T`) is what it points to.
func shapeOf(literal *ast.CompositeLit, info *types.Info) literalShape {
	of := info.TypeOf(literal)
	if of == nil {
		return otherLiteral
	}
	switch pointee(of).Underlying().(type) {
	case *types.Slice:
		return listLiteral
	case *types.Map:
		return mapLiteral
	case *types.Array:
		if array, written := literal.Type.(*ast.ArrayType); written {
			if _, elided := array.Len.(*ast.Ellipsis); elided {
				return listLiteral
			}
		}
	}
	return otherLiteral
}

// lastRemoved removes a list's last item, with the comma before it; or everything between
// the braces of a list of one item.
func lastRemoved(literal *ast.CompositeLit) literalMutant {
	count := len(literal.Elts)
	last := literal.Elts[count-1]
	start, end := literal.Lbrace+1, literal.Rbrace
	if count > 1 {
		start, end = literal.Elts[count-2].End(), last.End()
	}
	return literalMutant{start: start, end: end, description: itemText(last) + " removed"}
}

// itemText is an item of a composite literal as types.ExprString writes an expression,
// which writes neither a key nor a literal whose type is elided.
func itemText(item ast.Expr) string {
	switch x := item.(type) {
	case *ast.KeyValueExpr:
		return itemText(x.Key) + ": " + itemText(x.Value)
	case *ast.CompositeLit:
		if x.Type == nil && len(x.Elts) > 0 {
			return "{…}"
		}
		if x.Type == nil {
			return "{}"
		}
	}
	return types.ExprString(item)
}

// basic collects the change of a number or a string.
func (w *mutantWalk) basic(literal *ast.BasicLit) {
	if replacement, changed := changedLiteral(literal); changed {
		w.found = append(w.found, literalMutant{
			start: literal.Pos(), end: literal.End(), replacement: replacement,
			description: literal.Value + " → " + replacement,
		})
	}
}

// changedLiteral is how a number or a string is written once changed; false for a rune
// or an imaginary number.
func changedLiteral(literal *ast.BasicLit) (string, bool) {
	switch literal.Kind {
	case token.INT:
		return plusOne(literal).ExactString(), true
	case token.FLOAT:
		return floatText(plusOne(literal)), true
	case token.STRING:
		closing := len(literal.Value) - 1
		return literal.Value[:closing] + "x" + literal.Value[closing:], true
	}
	return "", false
}

// plusOne is the value of a number plus one.
func plusOne(literal *ast.BasicLit) constant.Value {
	value := constant.MakeFromLiteral(literal.Value, literal.Kind, 0)
	return constant.BinaryOp(value, token.ADD, constant.MakeInt64(1))
}

// floatText writes a number so that Go reads it as a floating-point constant.
func floatText(value constant.Value) string {
	exact, _ := constant.Float64Val(value)
	text := strconv.FormatFloat(exact, 'g', -1, 64)
	if !strings.ContainsAny(text, ".e") {
		text += ".0"
	}
	return text
}

// boolean collects the change of the predeclared `true` or `false`.
func (w *mutantWalk) boolean(name *ast.Ident) {
	object := w.info.Uses[name]
	if object == nil || object.Parent() != types.Universe {
		return
	}
	flipped := map[string]string{"true": "false", "false": "true"}[name.Name]
	if flipped != "" {
		w.found = append(w.found, literalMutant{
			start: name.Pos(), end: name.End(), replacement: flipped, description: name.Name + " → " + flipped,
		})
	}
}
