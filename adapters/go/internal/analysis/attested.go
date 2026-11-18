package analysis

import (
	"bytes"
	"errors"
	"fmt"
	"go/build/constraint"
	"io/fs"
	"os"
	"path/filepath"
	"slices"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"golang.org/x/tools/go/packages"
)

// The code attestations name is loaded for `symbols` in one load: each block of a page as
// a program of its own, added to the module through the load's overlay, and the packages
// of the files examples and citations name, with their tests. Such a file is loaded with
// its build constraints blanked, so that a test behind a tag (`//go:build integration`)
// names what it uses as any other does; a file name that is for another platform
// (`_windows_test.go`) still keeps it out. A block is checked only when its program
// reaches a surface's package, by being one or importing one at any depth: no other code
// can name what a surface declares, and a block that imports only the standard library
// would otherwise cost the checking of all it imports.

// symbolsDir is the directory of the module, never written, that blocks are loaded in.
const symbolsDir = "docattest_symbols"

// attested is the code a set of attestations names, loaded and typed.
type attested struct {
	*loaded
	// blocks are the file each block of a page was loaded as, by the attestation's index.
	blocks map[int]string
}

// example is the file an example is in: its block's own, or the source file that
// declares it.
func (code attested) example(k int, page string) typedFile {
	if block, isBlock := code.blocks[k]; isBlock {
		return code.files[block]
	}
	return code.files[page]
}

// loadAttested loads the code that a set of attestations names.
func (p *Project) loadAttested(attestations []protocol.Attestation) (attested, error) {
	code := attested{blocks: map[int]string{}}
	r := request{overlay: map[string][]byte{}, tests: true}
	for k, a := range attestations {
		if named := sourceNamed(a); named != "" {
			if err := r.addFile(p.Abs(named)); err != nil {
				return attested{}, fmt.Errorf("cannot read %s: %w", named, err)
			}
		} else if a.Kind == protocol.KindExample {
			code.blocks[k] = r.addBlock(p.Module.Dir, k, a.Code)
		}
	}
	listed, err := p.loader.list(r)
	if err != nil {
		return attested{}, err
	}
	surfaces := reaching{dirs: p.surfaceDirs(), known: map[*packages.Package]bool{}}
	listed.drop(func(root *packages.Package) bool {
		return filepath.Dir(filepath.Clean(root.Dir)) == filepath.Join(p.Module.Dir, symbolsDir) && !surfaces.reach(root)
	})
	code.loaded = listed.check()
	return code, nil
}

// reaching tells the packages that reach one in some directories: that are in one of
// them, or import, at any depth, a package that is.
type reaching struct {
	dirs  []string
	known map[*packages.Package]bool
}

func (r reaching) reach(pkg *packages.Package) bool {
	if known, seen := r.known[pkg]; seen {
		return known
	}
	found := slices.Contains(r.dirs, filepath.Clean(pkg.Dir))
	for _, imported := range pkg.Imports {
		found = found || r.reach(imported)
	}
	r.known[pkg] = found
	return found
}

// sourceNamed is the source file an attestation names: a cited test's, or an example's
// when it is a testable example; "" for any other.
func sourceNamed(a protocol.Attestation) string {
	switch {
	case a.Kind == protocol.KindTest:
		return a.File
	case a.Kind == protocol.KindExample && language.IsSource(a.Page):
		return a.Page
	}
	return ""
}

// addBlock adds the k-th attestation's block of a page as a program of its own in a
// module, and is the file it is loaded as; "" when nothing of it parses.
func (r *request) addBlock(moduleDir string, k int, block string) string {
	form := language.Program(block, "block")
	if form.File == nil {
		return ""
	}
	dir := filepath.Join(moduleDir, symbolsDir, fmt.Sprintf("b%d", k))
	path := filepath.Join(dir, "block.go")
	r.overlay[path] = []byte(form.Source)
	r.patterns = append(r.patterns, dir)
	return path
}

// addFile adds the package of a source file, with its tests and with the file's build
// constraints blanked. A file that is not there adds nothing.
func (r *request) addFile(path string) error {
	text, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if lifted := unconstrained(text); !bytes.Equal(lifted, text) {
		r.overlay[path] = lifted
	}
	if dir := filepath.Dir(path); !slices.Contains(r.patterns, dir) {
		r.patterns = append(r.patterns, dir)
	}
	return nil
}

// unconstrained is a file's text with the build constraints above its package clause
// blanked, each line kept as an empty one so that every other line stays where it is.
func unconstrained(text []byte) []byte {
	lines := bytes.SplitAfter(text, []byte("\n"))
	for k, line := range lines {
		trimmed := string(bytes.TrimSpace(line))
		if constraint.IsGoBuild(trimmed) || constraint.IsPlusBuild(trimmed) {
			lines[k] = line[len(bytes.TrimRight(line, "\r\n")):]
		}
		if bytes.HasPrefix(line, []byte("package ")) {
			break
		}
	}
	return bytes.Join(lines, nil)
}
