package analysis

import (
	"bytes"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"go/types"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/toolchain"
	"golang.org/x/tools/go/packages"
)

// A project is read as the go command lists it, through golang.org/x/tools/go/packages,
// the loader gopls and staticcheck use, and checked by go/types. Every package is checked
// from source, what it imports too: export data is never read, since the project's go
// command may be newer than the Go this adapter was built with and write a format it
// cannot read, and nothing is compiled or run. Function bodies are checked in the module
// and in the surfaces' packages; elsewhere they are dropped before checking, since
// nothing a page attests is in them and they are most of the work. The errors of a
// package whose bodies were dropped are never looked at. Packages are listed with the
// tag of docs helper files (docsfiles.go), as docattest's own programs are built.
//
// The go command lists without compiling: a file that uses cgo is checked as it is
// written, its `C.` names unknown and never an error, which costs nothing a page attests
// and spares running cgo on every list. The test packages of a directory are checked
// here from the files the go command lists for it, and not listed by the go command,
// which would make each load list and plan the program that runs the tests (loader.go
// and testpackages.go say how).

// listMode is what the go command is asked of every package: what it is, its files and
// what it imports. Asking for types would make go/packages check them itself, once per
// load.
const listMode = packages.NeedName | packages.NeedFiles | packages.NeedImports | packages.NeedDeps |
	packages.NeedModule

// request is one load of packages.
type request struct {
	// patterns are the packages asked for, by directory.
	patterns []string
	// overlay replaces files, or adds them, by absolute path.
	overlay map[string][]byte
	// tests says whether the test packages of the patterns are loaded too.
	tests bool
}

// loaded is what one load gives.
type loaded struct {
	// byDir are its packages by directory, the package itself and never a test variant.
	byDir map[string]*goPackage
	// files are its files by path, tests included, each as the package that compiles it
	// typed it: a file that is not a test, as its own package did.
	files map[string]typedFile
}

// typedFile is a file as the type checker saw it.
type typedFile struct {
	syntax *ast.File
	fset   *token.FileSet
	info   *types.Info
	// testFiles are the `_test.go` files compiled in the same test package as `syntax`,
	// so citation symbol analysis can follow local test helpers without crossing into
	// production functions.
	testFiles []*ast.File
}

// load loads the packages a request asks for and every package they import. Its error
// is that the go command could not list them; a package that does not compile carries
// its own errors.
func (l *loader) load(r request) (*loaded, error) {
	listed, err := l.list(r)
	if err != nil {
		return nil, err
	}
	return listed.check(), nil
}

// listing is a load as the go command listed it, before any of it is checked.
type listing struct {
	loader  *loader
	roots   []*packages.Package
	overlay map[string][]byte
	tests   testSources
	sizes   types.Sizes
}

// list lists the packages a request asks for and every package they import, with what
// the test files a request asks for import.
func (l *loader) list(r request) (*listing, error) {
	out := &listing{loader: l, overlay: r.overlay, tests: testSources{}}
	if len(r.patterns) == 0 {
		return out, nil
	}
	if r.tests {
		out.tests = l.readTests(r)
	}
	config := &packages.Config{
		Mode: listMode, Dir: l.module.Dir, Env: loaderEnvironment(l.module, l.workspace), Overlay: r.overlay,
		BuildFlags: []string{"-tags=" + toolchain.DocsTag},
	}
	var err error
	if out.roots, err = packages.Load(config, append(slices.Clone(r.patterns), out.tests.imports()...)...); err != nil {
		return nil, fmt.Errorf("the go command could not list the packages: %w", err)
	}
	out.sizes, err = l.sizes()
	return out, err
}

// relist lists what an earlier listing of the working tree did, at the revision an
// overlay makes of it. That is the earlier listing itself when each file the overlay
// gives is on disk and begins as it does there, up to the end of its imports: the go
// command lists a package from its files' names, package clauses, build constraints and
// imports, which come first in a file, and from nothing that follows them that a check
// reads.
func (l *loader) relist(earlier *listing, r request) (*listing, error) {
	for path, text := range r.overlay {
		disk, err := os.ReadFile(path)
		if err != nil || !bytes.Equal(header(disk), header(text)) {
			return l.list(r)
		}
	}
	return &listing{loader: l, roots: earlier.roots, overlay: r.overlay, tests: earlier.tests, sizes: earlier.sizes}, nil
}

// header is a file's text up to the end of its imports, or of its package clause when
// it has none; all of it when it does not parse that far.
func header(text []byte) []byte {
	fset := token.NewFileSet()
	file, err := parser.ParseFile(fset, "", text, parser.ImportsOnly|parser.ParseComments)
	if err != nil {
		return text
	}
	end := file.Name.End()
	if len(file.Decls) > 0 {
		end = file.Decls[len(file.Decls)-1].End()
	}
	return text[:fset.Position(end).Offset]
}

// builds says whether the go command builds a package that a listing asked for from a
// directory: whether it lists a Go file of it for this platform.
func (in *listing) builds(dir string) bool {
	for _, root := range in.roots {
		if filepath.Clean(root.Dir) == dir && len(root.GoFiles) > 0 {
			return true
		}
	}
	return false
}

// drop leaves out of a listing the packages it asks for that are not needed, with what
// only they import.
func (in *listing) drop(unneeded func(root *packages.Package) bool) {
	in.roots = slices.DeleteFunc(in.roots, unneeded)
}

// check checks what a listing lists, and the test packages it asks for.
func (in *listing) check() *loaded {
	checked := in.loader.checkAll(in.roots, in.overlay, in.sizes)
	out := in.loader.indexed(checked)
	in.loader.addTests(out, checked, in.tests, in.sizes)
	return out
}

// nothingLoaded is a load of no package.
func nothingLoaded() *loaded {
	return &loaded{byDir: map[string]*goPackage{}, files: map[string]typedFile{}}
}

// loaderEnvironment is the go command's environment for a load: the adapter's own, with
// no package driver but the go command, in the project's workspace if it has one. A
// project with no go.mod is listed outside module mode, where a directory is a package
// as it is.
func loaderEnvironment(module Module, workspace toolchain.Workspace) []string {
	env := append(append(os.Environ(), "GOPACKAGESDRIVER=off"), workspace.Environment()...)
	if module.Path == "" {
		env = append(env, "GO111MODULE=off")
	}
	return env
}

// within says whether a path is a directory or in the tree under it.
func within(dir, path string) bool {
	rel, err := filepath.Rel(dir, path)
	return err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}

// indexed is a load's packages by directory and by file.
func (l *loader) indexed(checked map[*packages.Package]*checkedPackage) *loaded {
	out := nothingLoaded()
	for listed, pkg := range checked {
		if listed.Dir != "" {
			out.byDir[filepath.Clean(listed.Dir)] = newPackage(listed, pkg, l.fset)
		}
		l.addFiles(out, pkg.syntax, pkg.info)
	}
	return out
}

// addFiles adds files to a load's, as one check typed them.
func (l *loader) addFiles(out *loaded, files []*ast.File, info *types.Info) {
	for _, file := range files {
		out.files[l.fset.File(file.FileStart).Name()] = typedFile{syntax: file, fset: l.fset, info: info}
	}
}

// addTestFiles adds `_test.go` files to a load's, keeping the slice of test files of the
// same test package alongside each file.
func (l *loader) addTestFiles(out *loaded, files []*ast.File, info *types.Info) {
	for _, file := range files {
		out.files[l.fset.File(file.FileStart).Name()] = typedFile{
			syntax: file, fset: l.fset, info: info, testFiles: files,
		}
	}
}
