package analysis

import (
	"go/ast"
	"go/build"
	"go/parser"
	"go/token"
	"io"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

// goPackage is the package in one directory, read from syntax and never compiled.
type goPackage struct {
	Dir  string
	Name string
	Fset *token.FileSet
	// Files are its files that are not tests, by name; Tests are the others.
	Files, Tests map[string]*ast.File
}

// packages reads each directory once.
type packages struct {
	files Files
	lock  sync.Mutex
	read  map[string]*goPackage
}

// at is the package in a directory; nil when the directory cannot be read or holds no Go
// file that is built on this platform: either way there is no package to see. A file
// that cannot be read is not part of it.
func (p *packages) at(dir string) *goPackage {
	p.lock.Lock()
	defer p.lock.Unlock()
	if p.read == nil {
		p.read = map[string]*goPackage{}
	}
	if _, done := p.read[dir]; !done {
		p.read[dir] = p.readPackage(dir)
	}
	return p.read[dir]
}

func (p *packages) readPackage(dir string) *goPackage {
	names, err := p.files.List(dir)
	if err != nil {
		return nil
	}
	pkg := &goPackage{Dir: dir, Fset: token.NewFileSet(), Files: map[string]*ast.File{}, Tests: map[string]*ast.File{}}
	context := p.buildContext()
	for _, name := range names {
		if !strings.HasSuffix(name, ".go") {
			continue
		}
		if built, _ := context.MatchFile(dir, name); !built {
			continue
		}
		text, err := p.files.Read(filepath.Join(dir, name))
		if err != nil {
			continue
		}
		file, _ := parser.ParseFile(pkg.Fset, filepath.Join(dir, name), text, parser.ParseComments|parser.SkipObjectResolution)
		if file == nil {
			continue
		}
		if strings.HasSuffix(name, "_test.go") {
			pkg.Tests[name] = file
		} else {
			pkg.Files[name] = file
			pkg.Name = file.Name.Name
		}
	}
	if len(pkg.Files) == 0 {
		return nil
	}
	return pkg
}

// buildContext is the go command's view of this platform, reading files through Files:
// it tells which files of a directory are built here.
func (p *packages) buildContext() build.Context {
	context := build.Default
	context.OpenFile = func(path string) (io.ReadCloser, error) {
		text, err := p.files.Read(path)
		if err != nil {
			return nil, err
		}
		return io.NopCloser(strings.NewReader(text)), nil
	}
	return context
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
	// Value says that the name is a constant or a variable.
	Value bool
}

// declarations are the names a file declares at its top level, in order; a method is
// not one, and neither is the blank name.
func declarations(file *ast.File) []declared {
	out := []declared{}
	for _, decl := range file.Decls {
		switch d := decl.(type) {
		case *ast.FuncDecl:
			if d.Recv == nil {
				out = append(out, declared{Name: d.Name.Name})
			}
		case *ast.GenDecl:
			for _, spec := range d.Specs {
				out = append(out, specNames(spec)...)
			}
		}
	}
	return out
}

func specNames(spec ast.Spec) []declared {
	switch s := spec.(type) {
	case *ast.TypeSpec:
		return []declared{{Name: s.Name.Name}}
	case *ast.ValueSpec:
		out := []declared{}
		for _, name := range s.Names {
			if name.Name != "_" {
				out = append(out, declared{Name: name.Name, Value: true})
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
