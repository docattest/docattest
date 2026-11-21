package running

import (
	"bytes"
	"encoding/json"
	"go/ast"
	"go/parser"
	"go/token"
	"slices"
	"strconv"

	"github.com/docattest/docattest/adapters/go/internal/harness/program"
)

// decodeManifestFields parses `open.manifestFields` with the same number conversion the
// generated program uses, so each resolved field's concrete Go type matches at runtime.
func decodeManifestFields(raw json.RawMessage) map[string]map[string]any {
	if len(raw) == 0 {
		return nil
	}
	var fields map[string]map[string]any
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if err := decoder.Decode(&fields); err != nil {
		return nil
	}
	for _, entries := range fields {
		if _, err := program.DocattestNumbers(entries); err != nil {
			return nil
		}
	}
	return fields
}

// typedFieldCalls rewrites `manifestField("path", "keys")` calls in a Go fact expression
// to typed harness helpers (`docattestFieldString`, `docattestFieldInt`, …) according to
// the value's concrete type in `fields`. Calls directly inside a type assertion
// (`manifestField(...).(T)`) are kept as `manifestField` so existing assertions still
// compile.
func typedFieldCalls(expression string, fields map[string]map[string]any) string {
	if len(fields) == 0 {
		return expression
	}
	fset := token.NewFileSet()
	expr, err := parser.ParseExprFrom(fset, "", expression, 0)
	if err != nil {
		return expression
	}
	asserted := map[ast.Expr]bool{}
	ast.Inspect(expr, func(n ast.Node) bool {
		if assert, ok := n.(*ast.TypeAssertExpr); ok {
			asserted[assert.X] = true
		}
		return true
	})
	type replacement struct {
		start, end int
		name       string
	}
	var edits []replacement
	ast.Inspect(expr, func(n ast.Node) bool {
		call, ok := n.(*ast.CallExpr)
		if !ok || asserted[call] {
			return true
		}
		ident, helper := typedFieldHelper(call, fields)
		if ident != nil && helper != "" {
			edits = append(edits, replacement{
				start: fset.Position(ident.Pos()).Offset,
				end:   fset.Position(ident.End()).Offset,
				name:  helper,
			})
		}
		return true
	})
	if len(edits) == 0 {
		return expression
	}
	slices.SortFunc(edits, func(a, b replacement) int { return b.start - a.start })
	out := expression
	for _, edit := range edits {
		out = out[:edit.start] + edit.name + out[edit.end:]
	}
	return out
}

// typedFieldHelper returns the `manifestField` identifier and the typed helper name when
// `call` is `manifestField("path", "keys")` with a known concrete value in `fields`.
func typedFieldHelper(call *ast.CallExpr, fields map[string]map[string]any) (*ast.Ident, string) {
	ident, ok := call.Fun.(*ast.Ident)
	if !ok || ident.Name != "manifestField" || len(call.Args) != 2 {
		return nil, ""
	}
	path, ok1 := stringLiteral(call.Args[0])
	keys, ok2 := stringLiteral(call.Args[1])
	if !ok1 || !ok2 {
		return nil, ""
	}
	value, found := fields[path][keys]
	if !found {
		return nil, ""
	}
	return ident, helperForValue(value)
}

func stringLiteral(expr ast.Expr) (string, bool) {
	lit, ok := expr.(*ast.BasicLit)
	if !ok || lit.Kind != token.STRING {
		return "", false
	}
	text, err := strconv.Unquote(lit.Value)
	return text, err == nil
}

func helperForValue(value any) string {
	switch value.(type) {
	case string:
		return "docattestFieldString"
	case int64:
		return "docattestFieldInt"
	case float64:
		return "docattestFieldFloat"
	case bool:
		return "docattestFieldBool"
	case []any:
		return "docattestFieldSlice"
	case map[string]any:
		return "docattestFieldMap"
	}
	return ""
}
