package analysis

import (
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"os"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// `affected`: the surface exports a change to the code affects, and the constants a
// fact reads from source (see SourceOnly), which are attested as exports are.
//
// A package of the module is cut into units, one for each name it declares at its top
// level; a type's methods are part of the type's unit, and the constants of one
// declaration are one unit, since `iota` ties them together. A unit's fingerprint is
// its tokens, so reformatting and comments change nothing. A unit uses what the names in
// its code resolve to, as the type checker resolved them: a top-level name of a package
// of the module, and the type that declares a method or a field the code selects, so a
// method called through an interface is the interface's use (see collect). An export is
// affected when its unit changed, appeared or disappeared, or reaches such a unit
// through what it uses, in the working tree or at the base. The base is the working
// tree loaded with the changed files as they were.

// unitKey names a unit: a package's directory and a top-level name.
type unitKey struct{ dir, name string }

// unit is the code behind one top-level name.
type unit struct {
	fingerprint string
	uses        []unitKey
}

// revision is the module's code at one revision, as one load typed it.
type revision struct {
	units map[unitKey]*unit
	// names are the top-level names of each package of the module, in order.
	names map[string][]string
	// constants are the units of the names that declare constants.
	constants map[unitKey]bool
	// dirs are the directories of the module's packages, by import path.
	dirs map[string]string
}

// newRevision cuts the module's packages of a load into units. The overlay holds the
// text of the files that differ from the disk.
func newRevision(module Module, code *loaded, overlay map[string][]byte) (*revision, error) {
	r := &revision{units: map[unitKey]*unit{}, names: map[string][]string{}, constants: map[unitKey]bool{}, dirs: map[string]string{}}
	modulePackages := []*goPackage{}
	for dir := range code.byDir {
		if pkg := code.at(dir); pkg != nil && within(module.Dir, dir) {
			modulePackages = append(modulePackages, pkg)
			r.dirs[pkg.Path] = dir
		}
	}
	for _, pkg := range modulePackages {
		if err := r.add(pkg, overlay); err != nil {
			return nil, err
		}
	}
	return r, nil
}

// source is one file of a package as its units are read from it.
type source struct {
	fset *token.FileSet
	text string
	info *types.Info
}

// add reads a package into units.
func (r *revision) add(pkg *goPackage, overlay map[string][]byte) error {
	files := sorted(pkg.Files)
	for _, file := range files {
		for _, d := range declarations(file) {
			r.names[pkg.Dir] = append(r.names[pkg.Dir], d.Name)
			r.constants[unitKey{pkg.Dir, d.Name}] = d.What == Constant
		}
	}
	for _, file := range files {
		path := pkg.Fset.File(file.FileStart).Name()
		text, over := overlay[path]
		if !over {
			var err error
			if text, err = os.ReadFile(path); err != nil {
				return fmt.Errorf("cannot read %s: %w", path, err)
			}
		}
		in := &source{fset: pkg.Fset, text: string(text), info: pkg.Info}
		for _, decl := range file.Decls {
			r.addDecl(pkg.Dir, decl, in)
		}
	}
	return nil
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
			for _, declared := range specNames(spec, d.Tok) {
				names = append(names, declared.Name)
			}
			if d.Tok == token.CONST {
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

// addCode adds nodes to the unit of each owner: their tokens, and the units they use.
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
			r.units[key].uses = append(r.units[key].uses, r.uses(part, in.info)...)
		}
	}
}

func isNil(node ast.Node) bool {
	body, ok := node.(*ast.BlockStmt)
	return ok && body == nil
}

// fingerprint is a node's code as its tokens: neither layout nor comments are in it.
func (in *source) fingerprint(node ast.Node) string {
	file := in.fset.File(node.Pos())
	from, to := file.Offset(node.Pos()), file.Offset(node.End())
	return strings.Join(language.Spellings(in.text[from:to]), " ")
}

// uses are the units a node's names resolve to.
func (r *revision) uses(node ast.Node, info *types.Info) []unitKey {
	out := []unitKey{}
	ast.Inspect(node, func(n ast.Node) bool {
		var used types.Object
		switch x := n.(type) {
		case *ast.SelectorExpr:
			if selection := info.Selections[x]; selection != nil {
				used = owner(selection)
			}
		case *ast.Ident:
			used = info.Uses[x]
		}
		if used != nil && used.Pkg() != nil && used.Parent() == used.Pkg().Scope() && r.dirs[used.Pkg().Path()] != "" {
			out = append(out, unitKey{r.dirs[used.Pkg().Path()], used.Name()})
		}
		return true
	})
	return out
}

// Affected answers `affected`. The base holds the text of every changed file as it was,
// by project path; nil for a file the base did not have.
func (p *Project) Affected(base map[string]*string) ([]protocol.SymbolRef, error) {
	if p.treeFailed != nil {
		return nil, fmt.Errorf("cannot load the code of the surfaces: %w", p.treeFailed)
	}
	overlay := p.baseOverlay(base)
	listed, err := p.loader.relist(p.treeListing, p.treeRequest(overlay))
	if err != nil {
		return nil, fmt.Errorf("cannot load the code of the surfaces at the base: %w", err)
	}
	atBase := listed.check()
	now, err := newRevision(p.Module, p.tree(), nil)
	if err != nil {
		return nil, err
	}
	before, err := newRevision(p.Module, atBase, overlay)
	if err != nil {
		return nil, err
	}
	differing := differingUnits(now, before)
	touched := union(now.reaching(differing), before.reaching(differing))
	out := []protocol.SymbolRef{}
	for _, of := range p.surfaces {
		for _, name := range unionInOrder(now.names[of.Dir], before.names[of.Dir]) {
			key := unitKey{of.Dir, name}
			if of.Dir != "" && touched[key] && (ast.IsExported(name) || now.constants[key] || before.constants[key]) {
				out = append(out, protocol.SymbolRef{Name: name, Surface: of.Name})
			}
		}
	}
	return out, nil
}

// notAtBase is what a Go file the base did not have is at the base: a file no build
// includes.
const notAtBase = "//go:build ignore\n\npackage ignored\n"

// baseOverlay is the overlay that makes the working tree the base: each changed Go file
// as it was.
func (p *Project) baseOverlay(base map[string]*string) map[string][]byte {
	overlay := map[string][]byte{}
	for rel, text := range base {
		switch {
		case !language.IsSource(rel):
		case text == nil:
			overlay[p.Abs(rel)] = []byte(notAtBase)
		default:
			overlay[p.Abs(rel)] = []byte(*text)
		}
	}
	return overlay
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
