package language

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"regexp"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

var (
	proseComment = regexp.MustCompile(`(?s)<!--.*?-->`)
	whitespace   = regexp.MustCompile(`\s+`)
)

// IsSource says whether a project path is a Go source file.
func IsSource(page string) bool {
	return strings.HasSuffix(page, ".go")
}

// IsTestFile says whether a project path is a Go test file.
func IsTestFile(page string) bool {
	return strings.HasSuffix(page, "_test.go")
}

// prose is a doc comment as one line of prose, written as a claim is written: comments
// in the prose replaced by a space, whitespace collapsed, trimmed.
func prose(doc *ast.CommentGroup) string {
	if doc == nil {
		return ""
	}
	text := proseComment.ReplaceAllString(doc.Text(), " ")
	return strings.TrimSpace(whitespace.ReplaceAllString(text, " "))
}

// ParseSource parses a Go source file with its comments; the file is nil when nothing
// of it parses.
func ParseSource(text string) (*token.FileSet, *ast.File) {
	fset := token.NewFileSet()
	file, _ := parser.ParseFile(fset, "", text, parser.ParseComments|parser.SkipObjectResolution)
	return fset, file
}

// docProseOf lists the doc comments of a source file's declarations, in order.
func docProseOf(source protocol.SourceText) []protocol.DocProse {
	out := []protocol.DocProse{}
	if !IsSource(source.Page) {
		return out
	}
	fset, file := ParseSource(source.Text)
	if file == nil {
		return out
	}
	for _, doc := range docs(file) {
		out = append(out, protocol.DocProse{
			Text:  prose(doc),
			Start: fset.Position(doc.Pos()).Line,
			End:   fset.Position(doc.End()).Line,
		})
	}
	return out
}

// docs are the doc comments of a file: the package's, then each declaration's.
func docs(file *ast.File) []*ast.CommentGroup {
	found := []*ast.CommentGroup{file.Doc}
	for _, decl := range file.Decls {
		switch d := decl.(type) {
		case *ast.FuncDecl:
			found = append(found, d.Doc)
		case *ast.GenDecl:
			found = append(found, d.Doc)
			for _, spec := range d.Specs {
				found = append(found, specDoc(spec))
			}
		}
	}
	out := []*ast.CommentGroup{}
	for _, doc := range found {
		if doc != nil {
			out = append(out, doc)
		}
	}
	return out
}

func specDoc(spec ast.Spec) *ast.CommentGroup {
	switch s := spec.(type) {
	case *ast.TypeSpec:
		return s.Doc
	case *ast.ValueSpec:
		return s.Doc
	}
	return nil
}

// IsExample says whether a function is a testable example, as `go test` finds them: no
// receiver, no parameters, no results, and a name that is Example or goes on from it
// with anything but a lower-case letter.
func IsExample(fn *ast.FuncDecl) bool {
	if fn.Recv != nil || fn.Type.Params.NumFields() > 0 || fn.Type.Results.NumFields() > 0 {
		return false
	}
	return HasTestPrefix(fn.Name.Name, "Example")
}

// HasTestPrefix says whether a function name is `prefix`, or goes on from it with
// anything but a lower-case letter: how the go command tells a test from `Testify`.
func HasTestPrefix(name, prefix string) bool {
	rest, found := strings.CutPrefix(name, prefix)
	if !found {
		return false
	}
	first, _ := utf8.DecodeRuneInString(rest)
	return rest == "" || !unicode.IsLower(first)
}

// ExampleOwner is the declaration an example documents: `Add` for ExampleAdd and for
// ExampleAdd_negative, `Client` for ExampleClient_Get, and none for a package's example.
func ExampleOwner(name string) string {
	owner, _, _ := strings.Cut(strings.TrimPrefix(name, "Example"), "_")
	return owner
}

// docExamplesOf are the testable examples a test file declares, as attestations: one that
// states its output is checked, and one that does not is only compiled, as `go test`
// treats them. The claim an example defends is its own doc comment.
func docExamplesOf(source protocol.SourceText) []protocol.Example {
	out := []protocol.Example{}
	if !IsTestFile(source.Page) {
		return out
	}
	fset, file := ParseSource(source.Text)
	if file == nil {
		return out
	}
	for _, decl := range file.Decls {
		fn, ok := decl.(*ast.FuncDecl)
		if !ok || !IsExample(fn) {
			continue
		}
		mode := "compile"
		if _, _, states := Output(fset, file, fn); states {
			mode = "check"
		}
		start, end := fset.Position(fn.Pos()), fset.Position(fn.End())
		out = append(out, protocol.Example{
			ID:    fmt.Sprintf("%s#%s", source.Page, fn.Name.Name),
			Page:  source.Page,
			Line:  start.Line,
			Claim: prose(fn.Doc),
			Kind:  protocol.KindExample,
			Lang:  "go",
			Mode:  mode,
			Code:  source.Text[start.Offset:end.Offset],
			Owner: ExampleOwner(fn.Name.Name),
		})
	}
	return out
}
