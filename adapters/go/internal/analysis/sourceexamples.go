package analysis

import (
	"errors"
	"fmt"
	"go/ast"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/language"
)

// SourceExample is a testable example function of a source file.
type SourceExample struct {
	// Name is its function's name.
	Name string
	// Dir is its file's directory, and File the file's name.
	Dir, File string
	// Package is the name of the package it tests: an external test package's without
	// its `_test`.
	Package string
	// First and Last are the lines of its function in its file.
	First, Last int
}

// SourceExampleAt finds the testable example declared at a line of a source file
// again; nil when it is gone.
func (p *Project) SourceExampleAt(page string, line int) (*SourceExample, error) {
	fset, file, err := p.parse(page)
	fn := exampleAt(fset, file, line)
	if fn == nil {
		return nil, err
	}
	path := p.Abs(page)
	return &SourceExample{
		Name:    fn.Name.Name,
		Dir:     filepath.Dir(path),
		File:    filepath.Base(path),
		Package: strings.TrimSuffix(file.Name.Name, "_test"),
		First:   fset.Position(fn.Pos()).Line,
		Last:    fset.Position(fn.End()).Line,
	}, nil
}

// parse reads a source file of the project; the file is nil when it is not there or
// cannot be read, and the error says why when it is there.
func (p *Project) parse(page string) (*token.FileSet, *ast.File, error) {
	text, err := os.ReadFile(p.Abs(page))
	switch {
	case errors.Is(err, fs.ErrNotExist):
		return nil, nil, nil
	case err != nil:
		return nil, nil, fmt.Errorf("cannot read %s: %w", page, err)
	}
	fset, file := language.ParseSource(string(text))
	return fset, file, nil
}

// exampleAt is the example function declared at a line of a file.
func exampleAt(fset *token.FileSet, file *ast.File, line int) *ast.FuncDecl {
	if file == nil {
		return nil
	}
	for _, decl := range file.Decls {
		fn, ok := decl.(*ast.FuncDecl)
		if ok && language.IsExample(fn) && fset.Position(fn.Pos()).Line == line {
			return fn
		}
	}
	return nil
}
