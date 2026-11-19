package analysis

import (
	"crypto/sha256"
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/toolchain"
	"golang.org/x/tools/go/packages"
)

// loader loads the packages of one project for one session. Its loads share a file set
// and every package they checked, by what it was checked from: its files, the text an
// overlay gives them, and what it imports, as they were checked. A later load checks
// only what it adds or what an overlay changes, and a package two loads share is the
// same *types.Package in both, so that code of one load uses the types of another.
// Nothing changes the files of the project on disk while a session reads it.
type loader struct {
	module Module
	// workspace is the go.work the go command lists with; empty for none.
	workspace toolchain.Workspace
	// bodies are the directories outside the module whose function bodies are checked.
	bodies []string
	fset   *token.FileSet
	// sizes are the sizes of the platform the go command builds for.
	sizes func() (types.Sizes, error)

	lock    sync.Mutex
	checked map[packageKey]*checkedPackage
}

// newLoader is the loader of a module. It asks the go command what platform it builds
// for at once, while the first load lists.
func newLoader(module Module, workspace toolchain.Workspace, bodies []string) *loader {
	l := &loader{
		module: module, workspace: workspace, bodies: bodies,
		fset: token.NewFileSet(), checked: map[packageKey]*checkedPackage{},
		sizes: sync.OnceValues(func() (types.Sizes, error) { return targetSizes(module, workspace) }),
	}
	go func() { _, _ = l.sizes() }()
	return l
}

// targetSizes are the sizes of what the go command builds for: they decide, among
// others, what `unsafe.Sizeof` and an untyped constant converted to `int` give. They are
// asked of the go command the package loader lists with, the first on the PATH.
func targetSizes(module Module, workspace toolchain.Workspace) (types.Sizes, error) {
	asked := toolchain.Run(toolchain.Command{
		Path: "go", Args: []string{"env", "GOARCH"}, Dir: module.Dir, Env: workspace.Environment(),
	})
	switch {
	case asked.Err != nil:
		return nil, fmt.Errorf("the go command could not be run: %w", asked.Err)
	case asked.Code != 0:
		return nil, fmt.Errorf("the go command could not say what it builds for: %s", strings.TrimSpace(asked.Stderr))
	}
	return types.SizesFor("gc", strings.TrimSpace(asked.Stdout)), nil
}

// keepsBodies says whether the function bodies of the files in a directory are checked.
func (l *loader) keepsBodies(dir string) bool {
	return within(l.module.Dir, dir) || slices.Contains(l.bodies, dir)
}

// packageKey is what a package was checked from, hashed: its ID, its files with the
// text an overlay gives them, its module's Go version, and the keys of what it imports.
type packageKey [sha256.Size]byte

// checkedPackage is one package as go/types checked it.
type checkedPackage struct {
	// done is closed once the package is checked.
	done   chan struct{}
	types  *types.Package
	syntax []*ast.File
	// info is what the checker found of each name; nil where bodies are dropped.
	info *types.Info
	// errors are what parsing and checking found wrong; the go command's own are the
	// listing's.
	errors []packages.Error
}

// checkAll checks every package of a listing, or takes it from an earlier check of the
// same code. A package is checked as soon as what it imports is, so that packages that
// do not depend on each other are checked at once.
func (l *loader) checkAll(roots []*packages.Package, overlay map[string][]byte, sizes types.Sizes) map[*packages.Package]*checkedPackage {
	keys := map[*packages.Package]packageKey{}
	out := map[*packages.Package]*checkedPackage{}
	// What a package imports is visited before it, so its key and its check exist.
	packages.Visit(roots, nil, func(listed *packages.Package) {
		keys[listed] = keyOf(listed, keys, overlay)
		imports := map[string]*checkedPackage{}
		for path, imported := range listed.Imports {
			imports[path] = out[imported]
		}
		out[listed] = l.checkOnce(keys[listed], func(into *checkedPackage) {
			l.check(into, listed, ofLoad{overlay: overlay, imports: imports, sizes: sizes})
		})
	})
	for _, pkg := range out {
		<-pkg.done
	}
	return out
}

// keyOf is a package's key, from the keys of what it imports.
func keyOf(listed *packages.Package, keys map[*packages.Package]packageKey, overlay map[string][]byte) packageKey {
	hash := sha256.New()
	fmt.Fprintf(hash, "%q\n", listed.ID)
	if listed.Module != nil {
		fmt.Fprintf(hash, "go %q\n", listed.Module.GoVersion)
	}
	for _, file := range listed.GoFiles {
		fmt.Fprintf(hash, "file %q\n", file)
		if text, over := overlay[file]; over {
			fmt.Fprintf(hash, "overlay %d %s\n", len(text), text)
		}
	}
	paths := make([]string, 0, len(listed.Imports))
	for path := range listed.Imports {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	for _, path := range paths {
		imported := keys[listed.Imports[path]]
		fmt.Fprintf(hash, "import %q %x\n", path, imported[:])
	}
	var key packageKey
	hash.Sum(key[:0])
	return key
}

// checkOnce is the check of a key: an earlier one, or a new one that runs in the
// background.
func (l *loader) checkOnce(key packageKey, check func(*checkedPackage)) *checkedPackage {
	l.lock.Lock()
	defer l.lock.Unlock()
	if earlier := l.checked[key]; earlier != nil {
		return earlier
	}
	pkg := &checkedPackage{done: make(chan struct{})}
	l.checked[key] = pkg
	go func() {
		defer close(pkg.done)
		check(pkg)
	}()
	return pkg
}

// ofLoad is what checking a package of a load needs beyond the listing.
type ofLoad struct {
	overlay map[string][]byte
	// imports are the checks of what the package imports, by import path as written.
	imports map[string]*checkedPackage
	sizes   types.Sizes
}

// check parses a listed package's files, waits for what it imports, and checks it.
func (l *loader) check(into *checkedPackage, listed *packages.Package, with ofLoad) {
	if listed.PkgPath == "unsafe" {
		into.types, into.syntax = types.Unsafe, []*ast.File{}
		return
	}
	into.syntax, into.errors = l.parseAll(listed.GoFiles, with.overlay)
	for _, imported := range with.imports {
		<-imported.done
	}
	checked := l.typeCheck(packageSource{
		path: listed.PkgPath, name: listed.Name, module: listed.Module, files: into.syntax,
		importer: importerFunc(func(path string) (*types.Package, error) {
			if imported := with.imports[path]; imported != nil {
				return imported.types, nil
			}
			return nil, fmt.Errorf("the go command lists no package %s", path)
		}),
		sizes: with.sizes, recorded: l.keepsBodies(filepath.Clean(listed.Dir)),
	})
	into.types, into.info, into.errors = checked.types, checked.info, append(into.errors, checked.errors...)
}

// importerFunc is a types.Importer that is a function.
type importerFunc func(path string) (*types.Package, error)

func (f importerFunc) Import(path string) (*types.Package, error) { return f(path) }
