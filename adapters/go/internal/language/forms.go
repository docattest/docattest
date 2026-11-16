// Package language answers what needs no project: the greeting, and the questions about
// Go text (blocks, doc comments, code spans, fact expressions).
package language

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/scanner"
	"go/token"
	"regexp"
	"strings"
)

// Form is a block of a page as the Go file it is compiled as.
//
// A block is a whole file when it begins with a package clause. Otherwise it is import
// declarations followed by either declarations (it is given an empty main when it has
// none) or statements (which become the body of main). `//line` directives make every
// position the compiler and a panic report a line of the block, in a file named by the
// label.
type Form struct {
	Source string
	Fset   *token.FileSet
	// File is the parsed source; nil when nothing of it parses.
	File *ast.File
	// Main is the block's main function, when it has one.
	Main *ast.FuncDecl
}

var outputComment = regexp.MustCompile(`(?i)^[[:space:]]*(unordered )?output:`)

// Program reads a block's code as a Go file whose positions name `label`.
func Program(code, label string) Form {
	if beginsWithPackage(code) {
		return parsed(directive(label, 1) + code)
	}
	declarations := "package main\n" + directive(label, 1) + code + "\n"
	form := parsed(declarations)
	if firstError(declarations) == nil {
		if form.Main == nil {
			return parsed(declarations + "func main() {}\n")
		}
		return form
	}
	statements := asStatements(code, label)
	if before(furthest(declarations), furthest(statements)) {
		return parsed(statements)
	}
	return form
}

func directive(label string, line int) string {
	return fmt.Sprintf("//line %s:%d\n", label, line)
}

func beginsWithPackage(code string) bool {
	_, err := parser.ParseFile(token.NewFileSet(), "", code, parser.PackageClauseOnly)
	return err == nil
}

// asStatements puts what follows the block's imports into the body of main.
func asStatements(code, label string) string {
	cut := importsEnd(code)
	line := 1 + strings.Count(code[:cut], "\n")
	return "package main\n" + directive(label, 1) + code[:cut] + "\nfunc main() {\n" +
		directive(label, line) + code[cut:] + "\n}\n"
}

// importsEnd is the offset in a block where its leading import declarations end.
func importsEnd(code string) int {
	const clause = "package main\n"
	fset := token.NewFileSet()
	file, _ := parser.ParseFile(fset, "", clause+code, parser.ImportsOnly)
	if file == nil || len(file.Decls) == 0 {
		return 0
	}
	end := fset.Position(file.Decls[len(file.Decls)-1].End()).Offset - len(clause)
	return max(0, min(end, len(code)))
}

func parsed(source string) Form {
	fset := token.NewFileSet()
	file, _ := parser.ParseFile(fset, "", source, parser.ParseComments)
	form := Form{Source: source, Fset: fset, File: file}
	if file == nil {
		return form
	}
	for _, decl := range file.Decls {
		if fn, ok := decl.(*ast.FuncDecl); ok && fn.Recv == nil && fn.Name.Name == "main" {
			form.Main = fn
		}
	}
	return form
}

func firstError(source string) error {
	_, err := parser.ParseFile(token.NewFileSet(), "", source, parser.SkipObjectResolution)
	return err
}

// reach is how far into a block the parser gets: the line and column of its first error.
type reach struct{ line, column int }

// furthest is the block position of a source's first syntax error, or one past any
// position when it has none. `//line` directives make it a position in the block.
func furthest(source string) reach {
	list, ok := firstError(source).(scanner.ErrorList)
	if !ok || len(list) == 0 {
		return reach{line: int(^uint(0) >> 1)}
	}
	return reach{list[0].Pos.Line, list[0].Pos.Column}
}

func before(a, b reach) bool {
	return a.line < b.line || (a.line == b.line && a.column < b.column)
}

// Output is what a function says it prints: the text of the last comment in its body
// when that begins `Output:` or `Unordered output:`, as `go test` reads an example.
func Output(fset *token.FileSet, file *ast.File, fn *ast.FuncDecl) (text string, unordered, ok bool) {
	if file == nil || fn == nil || fn.Body == nil {
		return "", false, false
	}
	var last *ast.CommentGroup
	for _, group := range file.Comments {
		if group.Pos() > fn.Body.Lbrace && group.End() <= fn.Body.Rbrace {
			last = group
		}
	}
	if last == nil {
		return "", false, false
	}
	whole := last.Text()
	found := outputComment.FindStringSubmatchIndex(whole)
	if found == nil {
		return "", false, false
	}
	text = strings.TrimLeft(whole[found[1]:], " ")
	text = strings.TrimPrefix(text, "\n")
	return text, found[2] >= 0, true
}
