package analysis

import (
	"errors"
	"fmt"
	"go/ast"
	"go/token"
	"io/fs"
	"path"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// `affected`: the surface exports a change to the code affects.
//
// A package is cut into units, one for each name it declares at its top level; a
// type's methods are part of the type's unit, and the constants of one declaration are
// one unit, since `iota` ties them together. A unit's fingerprint is its tokens, so
// reformatting and comments change nothing. An export is affected when its unit
// changed, appeared or disappeared, or reaches such a unit through the names it uses,
// in the working tree or at the base. It is read from syntax: a method is found through
// its type, and an interface is not followed to what implements it.

// unitKey names a unit: a package's directory and a top-level name.
type unitKey struct{ dir, name string }

// unit is the code behind one top-level name.
type unit struct {
	fingerprint string
	uses        []unitKey
}

// revision is the module's code at one revision, as its Files read it.
type revision struct {
	module Module
	files  Files
	units  map[unitKey]*unit
	// names are the top-level names of each package read so far, in order.
	names map[string][]string
}

func newRevision(module Module, files Files) *revision {
	return &revision{module: module, files: files, units: map[unitKey]*unit{}, names: map[string][]string{}}
}

// sources are the texts of a directory's Go files that are not tests, by name; none
// when the directory is not there.
func (r *revision) sources(dir string) (map[string]string, error) {
	out := map[string]string{}
	names, err := r.files.List(dir)
	switch {
	case errors.Is(err, fs.ErrNotExist):
		return out, nil
	case err != nil:
		return nil, err
	}
	for _, name := range names {
		if !language.IsSource(name) || language.IsTestFile(name) {
			continue
		}
		text, err := r.files.Read(filepath.Join(dir, name))
		if err != nil {
			return nil, err
		}
		out[name] = text
	}
	return out, nil
}

// source is one file of a package as its units are read from it.
type source struct {
	fset    *token.FileSet
	text    string
	imports map[string]string
}

// load reads a package into units, and then the packages of the module it imports.
func (r *revision) load(dir string) error {
	if _, done := r.names[dir]; done {
		return nil
	}
	r.names[dir] = []string{}
	files := map[*ast.File]*source{}
	order := []*ast.File{}
	texts, err := r.sources(dir)
	if err != nil {
		return err
	}
	for _, name := range sortedKeys(texts) {
		fset, file := language.ParseSource(texts[name])
		if file == nil {
			continue
		}
		files[file], order = &source{fset: fset, text: texts[name]}, append(order, file)
		for _, d := range declarations(file) {
			r.names[dir] = append(r.names[dir], d.Name)
		}
	}
	for _, file := range order {
		if files[file].imports, err = r.imports(file); err != nil {
			return err
		}
		for _, decl := range file.Decls {
			r.addDecl(dir, decl, files[file])
		}
	}
	return nil
}

// imports maps the names a file imports packages of the module under to their
// directories, reading each such package on the way.
func (r *revision) imports(file *ast.File) (map[string]string, error) {
	out := map[string]string{}
	for _, spec := range file.Imports {
		imported, _ := strconv.Unquote(spec.Path.Value)
		dir := r.module.directory(imported)
		if dir == "" {
			continue
		}
		if err := r.load(dir); err != nil {
			return nil, err
		}
		local := path.Base(imported)
		if spec.Name != nil {
			local = spec.Name.Name
		}
		out[local] = dir
	}
	return out, nil
}

// addDecl adds a declaration's code to the units of the names it declares.
func (r *revision) addDecl(dir string, decl ast.Decl, in *source) {
	owners := []string{}
	var parts []ast.Node
	switch d := decl.(type) {
	case *ast.FuncDecl:
		owners, parts = []string{funcOwner(d)}, []ast.Node{d.Type, d.Body}
		if d.Recv != nil {
			parts = append(parts, d.Recv, d.Name)
		}
	case *ast.GenDecl:
		for _, spec := range d.Specs {
			names := []string{}
			for _, declared := range specNames(spec) {
				names = append(names, declared.Name)
			}
			if d.Tok.String() == "const" {
				owners, parts = append(owners, names...), append(parts, spec)
				continue
			}
			r.addCode(dir, names, []ast.Node{spec}, in)
		}
	}
	r.addCode(dir, owners, parts, in)
}

// funcOwner is the name whose unit a function is part of: its own, or its receiver's type.
func funcOwner(fn *ast.FuncDecl) string {
	if fn.Recv == nil || len(fn.Recv.List) == 0 {
		return fn.Name.Name
	}
	receiver := fn.Recv.List[0].Type
	for {
		switch t := receiver.(type) {
		case *ast.StarExpr:
			receiver = t.X
		case *ast.IndexExpr:
			receiver = t.X
		case *ast.IndexListExpr:
			receiver = t.X
		case *ast.Ident:
			return t.Name
		default:
			return fn.Name.Name
		}
	}
}

// addCode adds nodes to the unit of each owner: their tokens, and the units they name.
func (r *revision) addCode(dir string, owners []string, parts []ast.Node, in *source) {
	for _, owner := range owners {
		key := unitKey{dir, owner}
		if r.units[key] == nil {
			r.units[key] = &unit{}
		}
		for _, part := range parts {
			if part == nil || isNil(part) {
				continue
			}
			r.units[key].fingerprint += in.fingerprint(part) + "\n"
			r.units[key].uses = append(r.units[key].uses, r.uses(dir, part, in.imports)...)
		}
	}
}

func isNil(node ast.Node) bool {
	body, ok := node.(*ast.BlockStmt)
	return ok && body == nil
}

// fingerprint is a node's code as its tokens: neither layout nor comments are in it.
func (in *source) fingerprint(node ast.Node) string {
	from, to := in.fset.Position(node.Pos()).Offset, in.fset.Position(node.End()).Offset
	return strings.Join(language.Spellings(in.text[from:to]), " ")
}

// uses are the units a node names: a top-level name of its own package, or a name
// selected from an imported package of the module.
func (r *revision) uses(dir string, node ast.Node, imports map[string]string) []unitKey {
	own := map[string]bool{}
	for _, name := range r.names[dir] {
		own[name] = true
	}
	out := []unitKey{}
	ast.Inspect(node, func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.SelectorExpr:
			if base, ok := x.X.(*ast.Ident); ok && imports[base.Name] != "" {
				out = append(out, unitKey{imports[base.Name], x.Sel.Name})
				return false
			}
		case *ast.Ident:
			if own[x.Name] {
				out = append(out, unitKey{dir, x.Name})
			}
		}
		return true
	})
	return out
}

// Affected answers `affected`. The base holds the text of every changed file as it was,
// by project path; nil for a file the base did not have.
func (p *Project) Affected(base map[string]*string) ([]protocol.SymbolRef, error) {
	was := map[string]*string{}
	for rel, text := range base {
		was[p.Abs(rel)] = text
	}
	now, before := newRevision(p.Module, p.files), newRevision(p.Module, atBase{tree: p.files, was: was})
	for _, of := range p.surfaces {
		if of.Dir == "" {
			continue
		}
		for _, at := range []*revision{now, before} {
			if err := at.load(of.Dir); err != nil {
				return nil, fmt.Errorf("cannot read the code of surface %q: %w", of.Name, err)
			}
		}
	}
	differing := differingUnits(now, before)
	touched := union(now.reaching(differing), before.reaching(differing))
	out := []protocol.SymbolRef{}
	for _, of := range p.surfaces {
		for _, name := range unionInOrder(now.names[of.Dir], before.names[of.Dir]) {
			if of.Dir != "" && ast.IsExported(name) && touched[unitKey{of.Dir, name}] {
				out = append(out, protocol.SymbolRef{Name: name, Surface: of.Name})
			}
		}
	}
	return out, nil
}

// differingUnits are the units that are not the same code at two revisions, or are in
// only one of them.
func differingUnits(now, was *revision) map[unitKey]bool {
	differing := map[unitKey]bool{}
	for key, current := range now.units {
		if before := was.units[key]; before == nil || before.fingerprint != current.fingerprint {
			differing[key] = true
		}
	}
	for key := range was.units {
		if now.units[key] == nil {
			differing[key] = true
		}
	}
	return differing
}

// reaching are the units that are in a set or reach one of it through what they use.
func (r *revision) reaching(set map[unitKey]bool) map[unitKey]bool {
	found := map[unitKey]bool{}
	for key := range set {
		found[key] = true
	}
	for grew := true; grew; {
		grew = false
		for key, code := range r.units {
			if found[key] {
				continue
			}
			for _, used := range code.uses {
				if found[used] {
					found[key], grew = true, true
					break
				}
			}
		}
	}
	return found
}
