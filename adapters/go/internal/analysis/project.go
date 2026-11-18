// Package analysis is what is read of a project without running it: its module, its
// surfaces and what they export, the symbols an attestation uses, the tests a file
// declares, and the exports a change affects. The go command lists the project's
// packages and go/types checks them (load.go says how); the tests a file declares are
// read from its syntax alone.
package analysis

import (
	"path/filepath"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// Config is what `open` says of a project that reading it needs.
type Config struct {
	// Root is the project's directory.
	Root string
	// InMemory says that the project is only what `open` gave: nothing above Root is
	// looked at.
	InMemory bool
	Surfaces []protocol.ConfiguredSurface
	// Setup is the setup package as the config names it; empty when none is configured.
	Setup string
}

// Project is a project as the type checker reads it.
type Project struct {
	Root   string
	Module Module
	// Setup is the setup package as the config names it; empty when none is configured.
	Setup    string
	surfaces []*Surface
	loader   *loader
	// tree is the working tree's surfaces, setup package, and what they import, checked
	// in the background from when the project is read, while the requests that follow
	// list what they need; it waits for the check.
	tree func() *loaded
	// treeListing is the working tree as the go command listed it.
	treeListing *listing
	// treeFailed is why the working tree could not be loaded, when it could not.
	treeFailed error
}

// Read reads a project's module, lists its surfaces and its setup package, resolves the
// surfaces, and starts checking them. A surface entry that is neither a string nor an
// import and a path is an error; a go command that cannot list the packages leaves every
// surface unresolved, saying why.
func Read(config Config) (*Project, error) {
	p := &Project{Root: config.Root, Setup: config.Setup}
	entries, err := readEntries(config.Surfaces)
	if err != nil {
		return nil, err
	}
	if p.Module, err = findModule(p.moduleStart(entries), p.Root, config.InMemory); err != nil {
		return nil, err
	}
	for k, configured := range config.Surfaces {
		p.surfaces = append(p.surfaces, p.resolve(configured.Name, entries[k]))
	}
	p.loader = newLoader(p.Module, p.surfaceDirs())
	p.startTree()
	return p, nil
}

// startTree lists the working tree, leaves unresolved a surface the go command builds no
// package from, and checks the rest in the background.
func (p *Project) startTree() {
	listed, err := p.loader.list(p.treeRequest(nil))
	if err != nil {
		p.treeFailed, p.tree = err, nothingLoaded
		listed = &listing{}
	} else {
		p.tree = sync.OnceValue(listed.check)
		go p.tree()
	}
	p.treeListing = listed
	for _, found := range p.surfaces {
		if found.Dir != "" && !listed.builds(found.Dir) {
			found.Dir = ""
		}
	}
}

// treeRequest asks for the surfaces' packages and the setup package, at the revision an
// overlay makes of the working tree.
func (p *Project) treeRequest(overlay map[string][]byte) request {
	r := request{patterns: p.surfaceDirs(), overlay: overlay}
	if p.Setup != "" && listable(p.packageDir(p.Setup)) {
		r.patterns = append(r.patterns, p.packageDir(p.Setup))
	}
	return r
}

// surfaceDirs are the directories of the surfaces that resolved to one.
func (p *Project) surfaceDirs() []string {
	dirs := []string{}
	for _, of := range p.surfaces {
		if of.Dir != "" {
			dirs = append(dirs, of.Dir)
		}
	}
	return dirs
}

// Abs is the absolute path of a project path.
func (p *Project) Abs(rel string) string {
	return filepath.Join(p.Root, filepath.FromSlash(rel))
}

// rel is a path relative to the project root with `/` separators, or the path itself
// when it is outside the project.
func (p *Project) rel(path string) string {
	rel, err := filepath.Rel(p.Root, path)
	if err != nil || !within(p.Root, path) {
		return path
	}
	return filepath.ToSlash(rel)
}

// InModule is a path relative to the module, as the go command is given packages.
func (p *Project) InModule(path string) string {
	rel, err := filepath.Rel(p.Module.Dir, path)
	if err != nil {
		return path
	}
	return rel
}
