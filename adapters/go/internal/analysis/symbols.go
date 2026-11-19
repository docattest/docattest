package analysis

import (
	"go/ast"
	"go/types"
	"path/filepath"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// Symbols answers `symbols`: the surface symbols each attestation uses, by its id. A
// file an attestation names that is not there names nothing; one that cannot be read is
// an error.
func (p *Project) Symbols(attestations []protocol.Attestation) (map[string][]protocol.SymbolRef, error) {
	code, err := p.loadAttested(attestations)
	if err != nil {
		return nil, err
	}
	out := map[string][]protocol.SymbolRef{}
	for k, a := range attestations {
		found := p.newRefs(attestingOf(a.Kind))
		switch a.Kind {
		case protocol.KindExample:
			p.exampleSymbols(found, code.example(k, p.Abs(a.Page)), a)
		case protocol.KindFact:
			p.factSymbols(found, a)
		case protocol.KindTest:
			p.citationSymbols(found, code.files[p.Abs(a.File)], a)
		case protocol.KindCases:
			p.runnerSymbols(found, a.Runner)
		}
		out[a.ID] = found.found
	}
	return out, nil
}

// refs collects the surface symbols a piece of code uses, each once, in order.
type refs struct {
	project *Project
	// attesting is what the proof the code is attests of the names it reaches.
	attesting attesting
	seen      map[protocol.SymbolRef]bool
	found     []protocol.SymbolRef
	// followed are the values whose initializers were followed (values.go), each once.
	followed map[protocol.SymbolRef]bool
}

func (p *Project) newRefs(how attesting) *refs {
	return &refs{
		project: p, attesting: how, seen: map[protocol.SymbolRef]bool{}, found: []protocol.SymbolRef{},
		followed: map[protocol.SymbolRef]bool{},
	}
}

// add records a name of a surface, when the surface exports it.
func (r *refs) add(name string, of *Surface) {
	if r.project.Exports(of)[name] {
		r.record(name, of)
	}
}

// record records a name of a surface, once.
func (r *refs) record(name string, of *Surface) {
	ref := protocol.SymbolRef{Name: name, Surface: of.Name}
	if !r.seen[ref] {
		r.seen[ref] = true
		r.found = append(r.found, ref)
	}
}

// collect adds the surface symbols a piece of code uses, as the type checker resolved
// its names: a name a surface's package declares at its top level, however the code
// reaches it, and the type that declares a method or a field the code selects. A local
// name that shadows one of a surface's is not a use of it. A method called through an
// interface is the interface's: which type's method runs is known only when the program
// runs, and the code that made the value names that type where it made it.
func (r *refs) collect(node ast.Node, info *types.Info) {
	if node == nil || info == nil {
		return
	}
	ast.Inspect(node, func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.SelectorExpr:
			// What is selected is used after what it is selected from.
			if selection := info.Selections[x]; selection != nil {
				r.collect(x.X, info)
				r.addObject(owner(selection))
				return false
			}
		case *ast.Ident:
			r.addObject(info.Uses[x])
		}
		return true
	})
}

// addObject adds what a package declares at its top level, when the package is a
// surface's: as a value read (values.go), when the code reads values; else when the
// surface exports it.
func (r *refs) addObject(object types.Object) {
	if object == nil || object.Pkg() == nil || object.Parent() != object.Pkg().Scope() {
		return
	}
	for _, of := range r.project.surfacesOf(object.Pkg().Path()) {
		if r.attesting == valuesRead {
			r.readName(object.Name(), of)
		} else {
			r.add(object.Name(), of)
		}
	}
}

// surfacesOf are the surfaces whose package has an import path: the one examples import
// it by, or the one of the directory it is read from.
func (p *Project) surfacesOf(importPath string) []*Surface {
	out := []*Surface{}
	for _, of := range p.surfaces {
		if pkg := p.tree().at(of.Dir); pkg != nil && (of.Import == importPath || pkg.Path == importPath) {
			out = append(out, of)
		}
	}
	return out
}

// owner is the named type that declares what a selection selects: a method's receiver,
// or the struct a field is declared in, through any embedding; nil for one of a type
// with no name.
func owner(selection *types.Selection) types.Object {
	if method, isMethod := selection.Obj().(*types.Func); isMethod {
		return namedObject(method.Type().(*types.Signature).Recv().Type())
	}
	var declaring types.Object
	at := selection.Recv()
	for _, field := range selection.Index() {
		if named := namedObject(at); named != nil {
			declaring = named
		}
		fields, isStruct := pointee(at).Underlying().(*types.Struct)
		if !isStruct {
			break
		}
		at = fields.Field(field).Type()
	}
	return declaring
}

// namedObject is the declaration of a named type, or of the type a pointer points to;
// nil for a type with no name.
func namedObject(of types.Type) types.Object {
	if named, isNamed := types.Unalias(pointee(of)).(*types.Named); isNamed {
		return named.Origin().Obj()
	}
	return nil
}

// pointee is what a pointer type points to, or the type itself.
func pointee(of types.Type) types.Type {
	if pointer, isPointer := types.Unalias(of).(*types.Pointer); isPointer {
		return pointer.Elem()
	}
	return of
}

// factSymbols adds what a fact reads of its surface: the names it reads (values.go); and
// the helpers it calls, of the setup package or of the surface's docs helper files, with
// what each uses, as a cases table's runner is followed. A docs helper is attested itself
// too, so that a change to what it uses that is no export, which makes it affected,
// reaches the fact.
func (p *Project) factSymbols(found *refs, a protocol.Attestation) {
	of := p.SurfaceNamed(a.Surface)
	if of == nil || of.Dir == "" {
		return
	}
	surface, setup := p.tree().at(of.Dir), p.setupPackage()
	sourceOnly, docs, helpers := p.SourceOnly(of), surface.docsNames(), p.SetupExports()
	for _, name := range language.Names(a.Expression) {
		switch {
		case docs[name] != "" && sourceOnly[name] == "":
			found.record(name, of)
			found.collectReached(surface, surface.declarationOf(name), surface.docsGlue)
		case helpers[name] && sourceOnly[name] == "":
			found.collectReached(setup, setup.declarationOf(name), setup.exceptCases)
		default:
			found.readName(name, of)
		}
	}
}

// exampleSymbols adds what an example uses: the whole of a block of a page, or the
// function of a testable example in a source file, with the export it is named after.
func (p *Project) exampleSymbols(found *refs, code typedFile, a protocol.Attestation) {
	if !language.IsSource(a.Page) {
		found.collect(code.syntax, code.info)
		return
	}
	fn := exampleAt(code.fset, code.syntax, a.Line)
	if fn == nil {
		return
	}
	for _, of := range p.surfaces {
		if of.Dir == filepath.Dir(p.Abs(a.Page)) {
			found.add(language.ExampleOwner(fn.Name.Name), of)
		}
	}
	found.collect(fn, code.info)
}

func (p *Project) citationSymbols(found *refs, code typedFile, a protocol.Attestation) {
	if code.syntax == nil {
		return
	}
	if test := cited(declaredIn(code.fset, code.syntax), a.Title, a.Describes); test != nil {
		found.collect(test.Node, code.info)
	}
}

// casesVariable is the setup package's variable that gives the runner of each cases table.
const casesVariable = "Cases"

// runnerSymbols adds what the runner of a cases table uses: the function the setup
// package's Cases gives for it, and the setup package's own functions, methods,
// variables and constants that it uses, at any depth.
func (p *Project) runnerSymbols(found *refs, runner string) {
	setup := p.setupPackage()
	if setup == nil {
		return
	}
	for _, file := range sorted(setup.Files) {
		found.collectReached(setup, runnerBody(setup, file, runner), setup.exceptCases)
	}
}

// collectReached adds what a piece of a package's code uses, and what the package's own
// functions, methods, variables and constants it uses use, at any depth, of those that
// `follows` holds of. A piece that is not there (a type, which declares no code that
// runs) adds nothing.
func (r *refs) collectReached(pkg *goPackage, start ast.Node, follows func(decl ast.Node) bool) {
	if start == nil {
		return
	}
	for _, part := range pkg.reached(start, follows) {
		r.collect(part, pkg.Info)
	}
}

// runnerBody is the code of the runner a file's Cases literal gives for a name.
func runnerBody(setup *goPackage, file *ast.File, runner string) ast.Node {
	var body ast.Node
	ast.Inspect(file, func(n ast.Node) bool {
		spec, ok := n.(*ast.ValueSpec)
		if !ok || len(spec.Names) != 1 || spec.Names[0].Name != casesVariable || len(spec.Values) != 1 {
			return body == nil
		}
		literal, ok := spec.Values[0].(*ast.CompositeLit)
		if !ok {
			return false
		}
		for _, element := range literal.Elts {
			pair, ok := element.(*ast.KeyValueExpr)
			if ok && literalName(pair.Key) != nil && *literalName(pair.Key) == runner {
				body = setup.function(pair.Value)
			}
		}
		return false
	})
	return body
}

// function is the code a value of a package stands for: a function literal itself, or
// the declaration of the function a name refers to.
func (pkg *goPackage) function(value ast.Expr) ast.Node {
	name, ok := value.(*ast.Ident)
	if !ok {
		return value
	}
	if fn, isFunc := pkg.declaredBy[pkg.Info.Uses[name]].(*ast.FuncDecl); isFunc {
		return fn
	}
	return value
}
