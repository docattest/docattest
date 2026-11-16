package language

import (
	"go/ast"
	"go/token"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// placeholder stands for a name in an expression's shape; no Go token is spelled so.
const placeholder = "·"

// Asserts says whether a block can fail by its own doing: it states its output, or it
// calls something that ends the program. A block that cannot be read is not called weak.
func Asserts(code string) bool {
	form := Program(code, "block")
	if form.File == nil || firstError(form.Source) != nil {
		return true
	}
	if _, _, ok := Output(form.Fset, form.File, form.Main); ok {
		return true
	}
	found := false
	ast.Inspect(form.File, func(node ast.Node) bool {
		if call, ok := node.(*ast.CallExpr); ok && ends(call) {
			found = true
		}
		return !found
	})
	return found
}

// ends says whether a call ends the program: panic, log.Fatal and log.Panic and their
// kin, and os.Exit.
func ends(call *ast.CallExpr) bool {
	switch fn := call.Fun.(type) {
	case *ast.Ident:
		return fn.Name == "panic"
	case *ast.SelectorExpr:
		pkg, ok := fn.X.(*ast.Ident)
		if !ok {
			return false
		}
		name := fn.Sel.Name
		if pkg.Name == "log" {
			return strings.HasPrefix(name, "Fatal") || strings.HasPrefix(name, "Panic")
		}
		return pkg.Name == "os" && name == "Exit"
	}
	return false
}

// mentioned gives, for each code span, the first of `symbols` it names as an identifier.
func mentioned(spans, symbols []string) []*string {
	wanted := map[string]bool{}
	for _, symbol := range symbols {
		wanted[symbol] = true
	}
	out := make([]*string, len(spans))
	for k, span := range spans {
		tokens, _ := scan(span)
		for _, t := range tokens {
			if t.Kind == token.IDENT && wanted[t.Text] {
				name := t.Text
				out[k] = &name
				break
			}
		}
	}
	return out
}

// shapeOf is a fact expression with every identifier replaced by a placeholder, and the
// identifiers in order. An expression that is not Go's tokens is its own shape.
func shapeOf(expression string) protocol.ExpressionShape {
	tokens, ok := scan(expression)
	if !ok {
		return protocol.ExpressionShape{Shape: expression, Names: []string{}}
	}
	shape := protocol.ExpressionShape{Names: []string{}}
	parts := make([]string, len(tokens))
	for k, t := range tokens {
		parts[k] = t.Text
		if t.Kind == token.IDENT {
			parts[k] = placeholder
			shape.Names = append(shape.Names, t.Text)
		}
	}
	shape.Shape = strings.Join(parts, " ")
	return shape
}
