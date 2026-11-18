package analysis

import (
	"errors"
	"go/ast"
	"go/parser"
	"go/scanner"
	"go/types"
	"os"
	"path/filepath"
	"slices"
	"sync"

	"golang.org/x/tools/go/packages"
)

// packageSource is what one package is checked from.
type packageSource struct {
	// path is its import path, and name its package name.
	path, name string
	// module is the module it is in, whose go line is the language version of its files;
	// nil outside module mode.
	module   *packages.Module
	files    []*ast.File
	importer types.Importer
	sizes    types.Sizes
	// recorded says whether what the checker finds of each name is kept.
	recorded bool
}

// checked is a package as go/types gave it, with what it found wrong.
type checked struct {
	types  *types.Package
	info   *types.Info
	errors []packages.Error
}

// typeCheck checks a package as go/packages would: the error the checker returns is one
// of the package's only when it reported none, and `import "C"` is a package of names
// the checker knows nothing of.
func (l *loader) typeCheck(from packageSource) checked {
	out := checked{types: types.NewPackage(from.path, from.name)}
	config := &types.Config{
		Importer: from.importer, Sizes: from.sizes, FakeImportC: true,
		Error: func(err error) { out.errors = append(out.errors, problem(err)) },
	}
	if from.module != nil && from.module.GoVersion != "" {
		config.GoVersion = "go" + from.module.GoVersion
	}
	if from.recorded {
		out.info = &types.Info{
			Types:        map[ast.Expr]types.TypeAndValue{},
			Defs:         map[*ast.Ident]types.Object{},
			Uses:         map[*ast.Ident]types.Object{},
			Implicits:    map[ast.Node]types.Object{},
			Instances:    map[*ast.Ident]types.Instance{},
			Scopes:       map[ast.Node]*types.Scope{},
			Selections:   map[*ast.SelectorExpr]*types.Selection{},
			FileVersions: map[*ast.File]string{},
		}
	}
	if err := types.NewChecker(config, l.fset, out.types, out.info).Files(from.files); err != nil && len(out.errors) == 0 {
		out.errors = append(out.errors, problem(err))
	}
	return out
}

// problem is an error of parsing or checking as go/packages reports it.
func problem(err error) packages.Error {
	var typeError types.Error
	var pathError *os.PathError
	switch {
	case errors.As(err, &typeError):
		return packages.Error{Pos: typeError.Fset.Position(typeError.Pos).String(), Msg: typeError.Msg, Kind: packages.TypeError}
	case errors.As(err, &pathError):
		return packages.Error{Pos: pathError.Path + ":1", Msg: pathError.Err.Error(), Kind: packages.ParseError}
	}
	return packages.Error{Pos: "-", Msg: err.Error(), Kind: packages.UnknownError}
}

// parseAll parses files, each as an overlay gives it or as it is on disk, and says what
// keeps any from parsing. A file that parses only in part is kept as far as it parses.
// The files are parsed at once, since a package's files are most of what checking it
// waits for.
func (l *loader) parseAll(paths []string, overlay map[string][]byte) ([]*ast.File, []packages.Error) {
	parsed := make([]*ast.File, len(paths))
	found := make([][]packages.Error, len(paths))
	var all sync.WaitGroup
	for k, path := range paths {
		all.Add(1)
		go func() {
			defer all.Done()
			parsed[k], found[k] = l.parseFile(path, overlay)
		}()
	}
	all.Wait()
	files := []*ast.File{}
	for _, file := range parsed {
		if file != nil {
			files = append(files, file)
		}
	}
	return files, slices.Concat(found...)
}

// parseFile parses one file as parseAll does.
func (l *loader) parseFile(path string, overlay map[string][]byte) (*ast.File, []packages.Error) {
	text, over := overlay[path]
	if !over {
		var err error
		if text, err = os.ReadFile(path); err != nil {
			return nil, []packages.Error{problem(err)}
		}
	}
	file, err := l.parse(path, text)
	return file, parseProblems(err)
}

// parseProblems are what keeps a file from parsing, as go/packages reports them.
func parseProblems(err error) []packages.Error {
	var list scanner.ErrorList
	switch {
	case err == nil:
		return nil
	case !errors.As(err, &list):
		return []packages.Error{problem(err)}
	}
	out := []packages.Error{}
	for _, each := range list {
		out = append(out, packages.Error{Pos: each.Pos.String(), Msg: each.Msg, Kind: packages.ParseError})
	}
	return out
}

// parse parses a file, without its function bodies when they are not checked.
func (l *loader) parse(path string, text []byte) (*ast.File, error) {
	file, err := parser.ParseFile(l.fset, path, text, parser.ParseComments|parser.SkipObjectResolution)
	if file == nil || l.keepsBodies(filepath.Dir(path)) {
		return file, err
	}
	for _, decl := range file.Decls {
		if fn, isFunc := decl.(*ast.FuncDecl); isFunc {
			fn.Body = nil
		}
	}
	return file, err
}
