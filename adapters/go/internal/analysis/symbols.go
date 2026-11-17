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

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// refs collects the surface symbols a piece of code names, each once, in order.
type refs struct {
	project *Project
	seen    map[protocol.SymbolRef]bool
	found   []protocol.SymbolRef
}

func (p *Project) newRefs() *refs {
	return &refs{project: p, seen: map[protocol.SymbolRef]bool{}, found: []protocol.SymbolRef{}}
}

// add records a name of a surface, when the surface exports it.
func (r *refs) add(name string, of *Surface) {
	ref := protocol.SymbolRef{Name: name, Surface: of.Name}
	if r.seen[ref] || !r.project.Exports(of)[name] {
		return
	}
	r.seen[ref] = true
	r.found = append(r.found, ref)
}

// scope is how one file reaches the surfaces: by the names it imports them under, and
// without a name when it is a file of a surface's own package or imports it with a dot.
type scope struct {
	qualified map[string][]*Surface
	bare      []*Surface
}

// scopeOf reads a file's imports; `dir` is the file's directory, or "" for a block.
func (p *Project) scopeOf(file *ast.File, dir string) scope {
	found := scope{qualified: map[string][]*Surface{}}
	for _, spec := range file.Imports {
		imported, err := strconv.Unquote(spec.Path.Value)
		if err != nil {
			continue
		}
		for _, candidate := range p.surfaces {
			if candidate.Dir != "" && candidate.Import == imported {
				found.reach(p.localName(spec, candidate), candidate)
			}
		}
	}
	for _, candidate := range p.surfaces {
		if dir != "" && candidate.Dir == dir && p.packages.at(dir).Name == file.Name.Name {
			found.bare = append(found.bare, candidate)
		}
	}
	return found
}

// reach notes that a file knows a surface by a local name: none for a blank import,
// every name of the surface for a dot.
func (in *scope) reach(local string, of *Surface) {
	switch local {
	case "_":
	case ".":
		in.bare = append(in.bare, of)
	default:
		in.qualified[local] = append(in.qualified[local], of)
	}
}

// localName is the name a file knows an imported surface by.
func (p *Project) localName(spec *ast.ImportSpec, of *Surface) string {
	if spec.Name != nil {
		return spec.Name.Name
	}
	if pkg := p.packages.at(of.Dir); pkg != nil {
		return pkg.Name
	}
	return path.Base(of.Import)
}

// collect adds the surface symbols a node names, as its file's scope reaches them.
func (r *refs) collect(node ast.Node, in scope) {
	ast.Inspect(node, func(n ast.Node) bool {
		switch x := n.(type) {
		case *ast.SelectorExpr:
			if base, ok := x.X.(*ast.Ident); ok && in.qualified[base.Name] != nil {
				for _, of := range in.qualified[base.Name] {
					r.add(x.Sel.Name, of)
				}
				return false
			}
			r.collect(x.X, in)
			return false
		case *ast.Ident:
			for _, of := range in.bare {
				r.add(x.Name, of)
			}
		}
		return true
	})
}

// Symbols answers `symbols` for one attestation. A file it names that is not there
// names nothing; one that cannot be read is an error.
func (p *Project) Symbols(a protocol.Attestation) ([]protocol.SymbolRef, error) {
	found := p.newRefs()
	var err error
	switch a.Kind {
	case protocol.KindExample:
		err = p.exampleSymbols(found, a)
	case protocol.KindFact:
		if of := p.SurfaceNamed(a.Surface); of != nil {
			for _, name := range language.Names(a.Expression) {
				found.add(name, of)
			}
		}
	case protocol.KindTest:
		err = p.citationSymbols(found, a)
	case protocol.KindCases:
		p.runnerSymbols(found, a.Runner)
	}
	return found.found, err
}

func (p *Project) exampleSymbols(found *refs, a protocol.Attestation) error {
	if !language.IsSource(a.Page) {
		if form := language.Program(a.Code, "block"); form.File != nil {
			found.collect(form.File, p.scopeOf(form.File, ""))
		}
		return nil
	}
	fset, file, err := p.parse(a.Page)
	if fn := exampleAt(fset, file, a.Line); fn != nil {
		dir := filepath.Dir(p.Abs(a.Page))
		in := p.scopeOf(file, dir)
		for _, of := range p.surfaces {
			if of.Dir == dir {
				found.add(language.ExampleOwner(fn.Name.Name), of)
			}
		}
		found.collect(fn, in)
	}
	return err
}

// parse reads a source file of the project; the file is nil when it is not there or
// cannot be read, and the error says why when it is there.
func (p *Project) parse(page string) (*token.FileSet, *ast.File, error) {
	text, err := p.files.Read(p.Abs(page))
	switch {
	case errors.Is(err, fs.ErrNotExist):
		return nil, nil, nil
	case err != nil:
		return nil, nil, fmt.Errorf("cannot read %s: %w", page, err)
	}
	fset, file := language.ParseSource(text)
	return fset, file, nil
}

func (p *Project) citationSymbols(found *refs, a protocol.Attestation) error {
	fset, file, err := p.parse(a.File)
	if file == nil {
		return err
	}
	if test := cited(declaredIn(fset, file), a.Title, a.Describes); test != nil {
		found.collect(test.Node, p.scopeOf(file, filepath.Dir(p.Abs(a.File))))
	}
	return nil
}

// casesVariable is the setup package's variable that gives the runner of each cases table.
const casesVariable = "Cases"

// runnerSymbols adds what the runner of a cases table names: the function the setup
// package's Cases gives for it, and the setup package's own functions, variables and
// constants that it names, at any depth, each as its own file's imports reach the surfaces.
func (p *Project) runnerSymbols(found *refs, runner string) {
	setup := p.setupPackage()
	if setup == nil {
		return
	}
	for _, file := range sorted(setup.Files) {
		body := runnerBody(setup, file, runner)
		if body == nil {
			continue
		}
		for _, part := range setup.reached(code{node: body, file: file}, casesVariable) {
			found.collect(part.node, p.scopeOf(part.file, setup.Dir))
		}
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
	if fn, isFunc := pkg.topLevel[name.Name].node.(*ast.FuncDecl); isFunc {
		return fn
	}
	return value
}
