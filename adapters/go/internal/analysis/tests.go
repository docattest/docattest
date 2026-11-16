package analysis

import (
	"errors"
	"fmt"
	"go/ast"
	"go/token"
	"io/fs"
	"strconv"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// declaredTest is something a test file declares: a test (a Test, Fuzz or Example
// function, or a subtest with a literal name), or a benchmark, which is not one.
type declaredTest struct {
	Title string
	// Describes are the names that enclose it, outermost first; nil where one is computed.
	Describes []*string
	Line      int
	// Suite is what it is when it is not a test, with its article.
	Suite string
	// Node is its code: the function, or a subtest's function literal.
	Node ast.Node
}

// declaredIn lists what a test file declares, in order.
func declaredIn(fset *token.FileSet, file *ast.File) []declaredTest {
	out := []declaredTest{}
	for _, decl := range file.Decls {
		fn, ok := decl.(*ast.FuncDecl)
		if !ok || fn.Recv != nil {
			continue
		}
		name := fn.Name.Name
		top := declaredTest{Title: name, Describes: []*string{}, Line: fset.Position(fn.Pos()).Line, Node: fn}
		switch {
		case language.HasTestPrefix(name, "Benchmark"):
			top.Suite = "a benchmark"
			out = append(out, top)
		case language.HasTestPrefix(name, "Test"), language.HasTestPrefix(name, "Fuzz"), language.IsExample(fn):
			out = append(out, top)
			out = append(out, subtests(fset, fn.Body, []*string{&name})...)
		}
	}
	return out
}

// subtests are the subtests a body starts with `Run(name, func…)`, and theirs.
func subtests(fset *token.FileSet, body ast.Node, enclosing []*string) []declaredTest {
	out := []declaredTest{}
	if body == nil {
		return out
	}
	ast.Inspect(body, func(node ast.Node) bool {
		call, ok := node.(*ast.CallExpr)
		if !ok {
			return true
		}
		literal, isRun := subtestBody(call)
		if !isRun {
			return true
		}
		title := literalName(call.Args[0])
		if title != nil {
			out = append(out, declaredTest{
				Title: *title, Describes: enclosing, Line: fset.Position(call.Pos()).Line, Node: literal,
			})
		}
		below := append(append([]*string{}, enclosing...), title)
		out = append(out, subtests(fset, literal.Body, below)...)
		return false
	})
	return out
}

// subtestBody is the function a call runs as a subtest: `x.Run(name, func…)`.
func subtestBody(call *ast.CallExpr) (*ast.FuncLit, bool) {
	selector, ok := call.Fun.(*ast.SelectorExpr)
	if !ok || selector.Sel.Name != "Run" || len(call.Args) != 2 {
		return nil, false
	}
	literal, ok := call.Args[1].(*ast.FuncLit)
	return literal, ok
}

// literalName is a subtest's name when it is written as a string literal.
func literalName(arg ast.Expr) *string {
	literal, ok := arg.(*ast.BasicLit)
	if !ok || literal.Kind != token.STRING {
		return nil
	}
	name, err := strconv.Unquote(literal.Value)
	if err != nil {
		return nil
	}
	return &name
}

// ListTests answers `tests`: what a test file declares, read and never run.
func (p *Project) ListTests(file string) protocol.TestListing {
	text, err := p.files.Read(p.Abs(file))
	switch {
	case errors.Is(err, fs.ErrNotExist):
		return protocol.TestListing{Message: fmt.Sprintf("%s does not exist", file)}
	case err != nil:
		return protocol.TestListing{Message: fmt.Sprintf("%s cannot be read: %v", file, err)}
	}
	fset, parsed := language.ParseSource(text)
	if parsed == nil {
		return protocol.TestListing{Message: fmt.Sprintf("%s does not parse as Go", file)}
	}
	tests, suites := []protocol.DeclaredTest{}, []protocol.DeclaredSuite{}
	for _, d := range declaredIn(fset, parsed) {
		if d.Suite != "" {
			suites = append(suites, protocol.DeclaredSuite{Title: d.Title, Describes: d.Describes, Line: d.Line, Kind: d.Suite})
		} else {
			tests = append(tests, protocol.DeclaredTest{Title: d.Title, Describes: d.Describes, Line: d.Line})
		}
	}
	return protocol.TestListing{OK: true, Tests: &tests, Suites: &suites}
}

// cited is the test a citation names: the title, and the enclosing names when the
// citation gives any.
func cited(all []declaredTest, title string, describes []string) *declaredTest {
	for k, d := range all {
		if d.Suite == "" && d.Title == title && (len(describes) == 0 || sameNames(d.Describes, describes)) {
			return &all[k]
		}
	}
	return nil
}

func sameNames(declared []*string, cited []string) bool {
	if len(declared) != len(cited) {
		return false
	}
	for k, name := range declared {
		if name == nil || *name != cited[k] {
			return false
		}
	}
	return true
}
