package analysis

import (
	"go/ast"
	"go/token"
	"go/types"
	"path/filepath"
	"slices"
	"sort"

	"golang.org/x/tools/go/packages"
)

// goPackage is the package in one directory, as the go command lists it for this
// platform and go/types checks it.
type goPackage struct {
	Dir  string
	Name string
	// Path is its import path.
	Path string
	// Files are the files it is compiled from, by name; a test is not one of them.
	Files map[string]*ast.File
	Fset  *token.FileSet
	Types *types.Package
	Info  *types.Info
	// Errors are what the go command and the type checker found wrong with it.
	Errors []packages.Error
	// declaredBy is the code that declares each of its top-level functions, methods,
	// variables and constants: the function, or the spec that names it.
	declaredBy map[types.Object]ast.Node
}

// newPackage is a listed package as it was checked: its errors are the go command's,
// then the checker's.
func newPackage(listed *packages.Package, checked *checkedPackage, fset *token.FileSet) *goPackage {
	pkg := &goPackage{
		Dir: filepath.Clean(listed.Dir), Name: listed.Name, Path: listed.PkgPath, Files: map[string]*ast.File{},
		Fset: fset, Types: checked.types, Info: checked.info,
		Errors: append(slices.Clone(listed.Errors), checked.errors...),
	}
	for _, file := range checked.syntax {
		pkg.Files[filepath.Base(fset.File(file.FileStart).Name())] = file
	}
	pkg.declaredBy = pkg.declarationsByObject()
	return pkg
}

// at is the package in a directory; nil when the go command finds no Go file of it that
// is built on this platform.
func (l *loaded) at(dir string) *goPackage {
	if pkg := l.byDir[dir]; pkg != nil && len(pkg.Files) > 0 {
		return pkg
	}
	return nil
}

// declarationsByObject maps each object a package declares at its top level, its
// methods included, to the code that declares it.
func (pkg *goPackage) declarationsByObject() map[types.Object]ast.Node {
	out := map[types.Object]ast.Node{}
	if pkg.Info == nil {
		return out
	}
	for _, file := range pkg.Files {
		for _, decl := range file.Decls {
			switch d := decl.(type) {
			case *ast.FuncDecl:
				out[pkg.Info.Defs[d.Name]] = d
			case *ast.GenDecl:
				for _, spec := range d.Specs {
					if values, isValue := spec.(*ast.ValueSpec); isValue {
						for _, name := range values.Names {
							out[pkg.Info.Defs[name]] = spec
						}
					}
				}
			}
		}
	}
	delete(out, nil)
	return out
}

// sorted are a package's files in the order of their names, which is the order the go
// command reads them in.
func sorted(files map[string]*ast.File) []*ast.File {
	names := make([]string, 0, len(files))
	for name := range files {
		names = append(names, name)
	}
	sort.Strings(names)
	out := make([]*ast.File, len(names))
	for k, name := range names {
		out[k] = files[name]
	}
	return out
}

// declared is one name a package declares at its top level.
type declared struct {
	Name string
	What Declares
}

// Declares is what a top-level name of a package is.
type Declares string

const (
	Constant Declares = "constant"
	Variable Declares = "variable"
	Function Declares = "function"
	Type     Declares = "type"
)

// isValue says whether a name is a constant or a variable.
func (d declared) isValue() bool { return d.What == Constant || d.What == Variable }

// declarations are the names a file declares at its top level, in order; a method is
// not one, and neither is the blank name.
func declarations(file *ast.File) []declared {
	out := []declared{}
	for _, decl := range file.Decls {
		switch d := decl.(type) {
		case *ast.FuncDecl:
			if d.Recv == nil {
				out = append(out, declared{Name: d.Name.Name, What: Function})
			}
		case *ast.GenDecl:
			for _, spec := range d.Specs {
				out = append(out, specNames(spec, d.Tok)...)
			}
		}
	}
	return out
}

// specNames are the names a spec of a `const`, `var` or `type` declaration declares.
func specNames(spec ast.Spec, keyword token.Token) []declared {
	switch s := spec.(type) {
	case *ast.TypeSpec:
		return []declared{{Name: s.Name.Name, What: Type}}
	case *ast.ValueSpec:
		what := Variable
		if keyword == token.CONST {
			what = Constant
		}
		out := []declared{}
		for _, name := range s.Names {
			if name.Name != "_" {
				out = append(out, declared{Name: name.Name, What: what})
			}
		}
		return out
	}
	return nil
}

// exported are the exported names a package declares, in the order of its files.
func (pkg *goPackage) exported() []declared {
	out := []declared{}
	for _, file := range sorted(pkg.Files) {
		for _, d := range declarations(file) {
			if token.IsExported(d.Name) {
				out = append(out, d)
			}
		}
	}
	return out
}
