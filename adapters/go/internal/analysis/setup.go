package analysis

import (
	"go/ast"
	"go/token"
	"go/types"
)

// The setup of a Go project is a package of its module, named in the config by its
// directory or by one of its files. It may declare
//
//	var Cases = map[string]func(row map[string]string) error{…}
//
// the runners of cases tables by name, `var Columns = map[string][]string{…}`, the
// columns each runner's table must have, and anything else exported, which fact
// expressions may use by name.

// setupPackage is the setup package; nil when none is configured or it holds no Go.
func (p *Project) setupPackage() *goPackage {
	if p.Setup == "" {
		return nil
	}
	return p.tree().at(p.packageDir(p.Setup))
}

// SetupImport is the import path of the setup package; empty when it is in no module.
func (p *Project) SetupImport() string {
	return p.Module.importPath(p.packageDir(p.Setup))
}

// SetupFound says whether the setup package is there to be imported.
func (p *Project) SetupFound() bool {
	return p.setupPackage() != nil && p.SetupImport() != ""
}

// SetupExports is the set of the setup package's exports.
func (p *Project) SetupExports() map[string]bool {
	set := map[string]bool{}
	if !p.SetupFound() {
		return set
	}
	for _, d := range p.setupPackage().exported() {
		set[d.Name] = true
	}
	return set
}

// SetupHelpers are the setup package's exported functions that can be called without
// arguments and return one value, in the order of its files: what `observables` calls,
// since they exist for pages to read. Nothing else is ever called to see what it returns.
func (p *Project) SetupHelpers() []string {
	names := []string{}
	if !p.SetupFound() {
		return names
	}
	for _, file := range sorted(p.setupPackage().Files) {
		for _, decl := range file.Decls {
			if f, ok := decl.(*ast.FuncDecl); ok && callableAlone(f) {
				names = append(names, f.Name.Name)
			}
		}
	}
	return names
}

// callableAlone says whether a function is exported, neither a method nor generic, takes no
// argument it must be given (it has none, or only a variadic one) and returns one value.
func callableAlone(f *ast.FuncDecl) bool {
	signature := f.Type
	if f.Recv != nil || !token.IsExported(f.Name.Name) || signature.TypeParams != nil {
		return false
	}
	if signature.Results.NumFields() != 1 {
		return false
	}
	params := signature.Params.List
	if len(params) == 0 {
		return true
	}
	_, variadic := params[0].Type.(*ast.Ellipsis)
	return len(params) == 1 && len(params[0].Names) <= 1 && variadic
}

// reached is a piece of code and the package's top-level functions, methods, variables
// and constants it uses, at any depth, in the order they are first used: a runner's work
// is also in the helpers it calls and the tables it reads. Only the declarations that
// `follows` holds of are followed.
func (pkg *goPackage) reached(start ast.Node, follows func(decl ast.Node) bool) []ast.Node {
	seen := map[ast.Node]bool{start: true}
	out := []ast.Node{start}
	visit := func(n ast.Node) bool {
		if name, isName := n.(*ast.Ident); isName {
			if decl := pkg.declaredBy[origin(pkg.Info.Uses[name])]; decl != nil && !seen[decl] && follows(decl) {
				seen[decl] = true
				out = append(out, decl)
			}
		}
		return true
	}
	for k := 0; k < len(out); k++ {
		ast.Inspect(out[k], visit)
	}
	return out
}

// origin is the object a generic one was instantiated from, or the object itself.
func origin(object types.Object) types.Object {
	switch generic := object.(type) {
	case *types.Func:
		return generic.Origin()
	case *types.Var:
		return generic.Origin()
	}
	return object
}

// exceptCases says whether a declaration of the setup package is followed from a helper
// or a runner: every one is, but for the Cases table, which names every runner.
func (pkg *goPackage) exceptCases(decl ast.Node) bool {
	return decl != pkg.declarationOf(casesVariable)
}
