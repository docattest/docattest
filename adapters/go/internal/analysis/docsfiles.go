package analysis

import (
	"go/ast"
	"go/build/constraint"
	"go/token"
	"maps"
	"path/filepath"
	"slices"

	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// A docs helper file is a file of a package whose `//go:build` line holds only when the
// tag docattest is set (`//go:build docattest`). The go command builds it only with that
// tag, and docattest lists packages and builds its own programs with it, so such a file
// may read the package's unexported names for the pages while no program of the project
// links it. What it declares is no export of a surface: `uncovered` never lists it.

// published are the exported names a package declares outside its docs helper files, in
// the order of its files: what its users can import.
func (pkg *goPackage) published() []declared {
	out := []declared{}
	for _, file := range sorted(pkg.Files) {
		if isDocsFile(file) {
			continue
		}
		for _, d := range declarations(file) {
			if token.IsExported(d.Name) {
				out = append(out, d)
			}
		}
	}
	return out
}

// docsNames are the names a package's docs helper files declare at their top level.
func (pkg *goPackage) docsNames() map[string]Declares {
	out := map[string]Declares{}
	for _, file := range pkg.Files {
		if isDocsFile(file) {
			for _, d := range declarations(file) {
				out[d.Name] = d.What
			}
		}
	}
	return out
}

// DocsHelperFile is a docs helper file of a surface's package in the project's module.
type DocsHelperFile struct {
	// Path is the file's absolute path, and Page its path relative to the project root.
	Path, Page string
}

// DocsHelperFiles are the docs helper files of the surfaces and of an in-module setup
// package whose packages are in the project's module, surface by surface in the config's
// order, each surface's in the order of their names. A package of another module is not
// the project's to vet here (a separate setup module is vetted on its own).
func (p *Project) DocsHelperFiles() []DocsHelperFile {
	out := []DocsHelperFile{}
	seen := map[string]bool{}
	addDir := func(dir string) {
		pkg := p.tree().at(dir)
		if dir == "" || pkg == nil || !within(p.Module.Dir, dir) || seen[dir] {
			return
		}
		seen[dir] = true
		for _, name := range slices.Sorted(maps.Keys(pkg.Files)) {
			if isDocsFile(pkg.Files[name]) {
				path := filepath.Join(dir, name)
				out = append(out, DocsHelperFile{Path: path, Page: p.rel(path)})
			}
		}
	}
	for _, found := range p.surfaces {
		addDir(found.Dir)
	}
	if p.Setup != "" && p.SetupModule.Dir == p.Module.Dir {
		addDir(p.packageDir(p.Setup))
	}
	return out
}

// InSetupModule reports whether `path` lies inside a separate setup module (and inside
// the project root), returning its project-relative path.
func (p *Project) InSetupModule(path string) (string, bool) {
	if p.SetupModule.Dir == "" || p.SetupModule.Dir == p.Module.Dir {
		return "", false
	}
	if !within(p.SetupModule.Dir, path) || !within(p.Root, path) {
		return "", false
	}
	return p.rel(path), true
}

// docsGlue says whether a declaration of a surface's package is followed from a docs
// helper: one of its docs helper files, or a value. A docs helper is glue, so the helpers
// of the docs files it calls are its code too; a function of the package's ordinary files
// is attested by its name, as one a fact calls is, and its code is not followed.
func (pkg *goPackage) docsGlue(decl ast.Node) bool {
	_, isFunction := decl.(*ast.FuncDecl)
	return !isFunction || isDocsFile(pkg.fileOf(decl))
}

// fileOf is the file of the package that holds a node.
func (pkg *goPackage) fileOf(node ast.Node) *ast.File {
	for _, file := range pkg.Files {
		if file.FileStart <= node.Pos() && node.Pos() < file.FileEnd {
			return file
		}
	}
	return nil
}

// isDocsFile says whether a file is a docs helper file: whether its `//go:build` line,
// above its package clause, is false whatever the other tags it names are, when the docs
// tag is not set.
func isDocsFile(file *ast.File) bool {
	line := buildLine(file)
	if line == nil {
		return false
	}
	others := []string{}
	for _, tag := range tagsOf(line) {
		if tag != toolchain.DocsTag && !slices.Contains(others, tag) {
			others = append(others, tag)
		}
	}
	// Each bit of `set` says whether one of the other tags is set.
	for set := 0; set < 1<<len(others); set++ {
		holds := line.Eval(func(tag string) bool {
			for k, other := range others {
				if tag == other {
					return set&(1<<k) != 0
				}
			}
			return false
		})
		if holds {
			return false
		}
	}
	return true
}

// buildLine is a file's `//go:build` constraint; nil when it has none.
func buildLine(file *ast.File) constraint.Expr {
	for _, group := range file.Comments {
		if group.Pos() >= file.Package {
			break
		}
		for _, comment := range group.List {
			if !constraint.IsGoBuild(comment.Text) {
				continue
			}
			if line, err := constraint.Parse(comment.Text); err == nil {
				return line
			}
		}
	}
	return nil
}

// tagsOf are the tags a constraint names.
func tagsOf(line constraint.Expr) []string {
	switch x := line.(type) {
	case *constraint.TagExpr:
		return []string{x.Tag}
	case *constraint.NotExpr:
		return tagsOf(x.X)
	case *constraint.AndExpr:
		return append(tagsOf(x.X), tagsOf(x.Y)...)
	case *constraint.OrExpr:
		return append(tagsOf(x.X), tagsOf(x.Y)...)
	}
	return nil
}
