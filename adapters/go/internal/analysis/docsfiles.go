package analysis

import (
	"go/ast"
	"go/build/constraint"
	"go/token"
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
