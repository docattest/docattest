package analysis

import (
	"cmp"
	"fmt"
	"go/ast"
	"go/types"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"strings"

	"golang.org/x/tools/go/packages"
)

// The test packages of a directory are checked as the go command builds them for
// `go test`: the package with its own test files, and the external test package
// (`p_test`) of the files that name it, which imports the first. The go command says
// which files a build leaves out; every other file whose name ends in `_test.go` and
// does not start with `_` or `.` is a test file. A package another one imports is the
// package without its tests, as the go command builds it unless that other package is
// itself a test. The errors of a test package are never looked at: what its code uses is
// read as far as the checker could read it.

// testSources are the test files of the directories a load asks for, by directory,
// parsed before the load lists so that what they import is listed with it.
type testSources map[string][]*ast.File

// readTests parses the test files in the directories a request asks for. A directory
// that cannot be listed has no test files to check: what is wrong with it is the
// listing's to say.
func (l *loader) readTests(r request) testSources {
	out := testSources{}
	for _, dir := range r.patterns {
		entries, err := os.ReadDir(dir)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if name := entry.Name(); !entry.IsDir() && isTestFile(name) {
				out[dir] = append(out[dir], l.parseTest(filepath.Join(dir, name), r.overlay)...)
			}
		}
	}
	return out
}

// isTestFile says whether the go command reads a file of a directory as a test file.
func isTestFile(name string) bool {
	return strings.HasSuffix(name, "_test.go") && !strings.HasPrefix(name, "_") && !strings.HasPrefix(name, ".")
}

// parseTest is a test file, as far as it parses; none when it cannot be read.
func (l *loader) parseTest(path string, overlay map[string][]byte) []*ast.File {
	text, over := overlay[path]
	if !over {
		var err error
		if text, err = os.ReadFile(path); err != nil {
			return nil
		}
	}
	if file, _ := l.parse(path, text); file != nil {
		return []*ast.File{file}
	}
	return nil
}

// imports are the import paths the test files import, each once.
func (t testSources) imports() []string {
	out := []string{}
	for _, files := range t {
		for _, file := range files {
			for _, spec := range file.Imports {
				path, err := strconv.Unquote(spec.Path.Value)
				if err == nil && path != "C" && !slices.Contains(out, path) {
					out = append(out, path)
				}
			}
		}
	}
	return out
}

// addTests checks the test packages of the directories with test files, and adds their
// test files to a load as those packages typed them.
func (l *loader) addTests(out *loaded, all map[*packages.Package]*checkedPackage, tests testSources, sizes types.Sizes) {
	byPath := map[string]*types.Package{}
	for listed, pkg := range all {
		byPath[listed.PkgPath] = pkg.types
	}
	for listed, pkg := range all {
		files := tests[filepath.Clean(listed.Dir)]
		if listed.Dir == "" || len(files) == 0 {
			continue
		}
		internal, external := l.builtTests(listed, files)
		tested := pkg.types
		if len(internal) > 0 {
			checked := l.typeCheck(packageSource{
				path: listed.PkgPath, name: cmp.Or(listed.Name, internal[0].Name.Name), module: listed.Module,
				files: append(slices.Clone(pkg.syntax), internal...), importer: testImporter(listed, byPath, nil),
				sizes: sizes, recorded: true,
			})
			tested = checked.types
			l.addFiles(out, internal, checked.info)
		}
		if len(external) > 0 {
			checked := l.typeCheck(packageSource{
				path: listed.PkgPath + "_test", name: external[0].Name.Name, module: listed.Module,
				files: external, importer: testImporter(listed, byPath, tested), sizes: sizes, recorded: true,
			})
			l.addFiles(out, external, checked.info)
		}
	}
}

// builtTests are the test files of a package that the go command builds, split into the
// package's own and its external test package's.
func (l *loader) builtTests(listed *packages.Package, files []*ast.File) (internal, external []*ast.File) {
	for _, file := range files {
		switch name := file.Name.Name; {
		case slices.Contains(listed.IgnoredFiles, l.fset.File(file.FileStart).Name()):
		case name != listed.Name && strings.HasSuffix(name, "_test"):
			external = append(external, file)
		default:
			internal = append(internal, file)
		}
	}
	return internal, external
}

// testImporter imports for a test package of a listed package: the package under test
// is `tested` (the package with its own test files), and any other import is the package
// the go command lists for that path.
func testImporter(listed *packages.Package, byPath map[string]*types.Package, tested *types.Package) types.Importer {
	return importerFunc(func(path string) (*types.Package, error) {
		if imported := listed.Imports[path]; imported != nil {
			path = imported.PkgPath
		}
		if path == listed.PkgPath && tested != nil {
			return tested, nil
		}
		if pkg := byPath[path]; pkg != nil {
			return pkg, nil
		}
		return nil, fmt.Errorf("the go command lists no package %s", path)
	})
}
