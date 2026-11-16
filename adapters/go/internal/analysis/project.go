// Package analysis is what is read of a project from syntax, never compiled or run: its
// module, its surfaces and what they export, the symbols an attestation uses, the tests a
// file declares, and the exports a change affects. It reads the project's files through
// a Files it is given.
package analysis

import (
	"path/filepath"

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

// Project is a project as read from syntax.
type Project struct {
	Root   string
	Module Module
	// Setup is the setup package as the config names it; empty when none is configured.
	Setup    string
	surfaces []*Surface
	files    Files
	packages packages
}

// Read reads a project's module and resolves its surfaces. A surface entry that is
// neither a string nor an import and a path is an error.
func Read(config Config, files Files) (*Project, error) {
	p := &Project{Root: config.Root, Setup: config.Setup, files: files, packages: packages{files: files}}
	entries, err := readEntries(config.Surfaces)
	if err != nil {
		return nil, err
	}
	if p.Module, err = findModule(files, p.moduleStart(entries), p.Root, config.InMemory); err != nil {
		return nil, err
	}
	for k, configured := range config.Surfaces {
		p.surfaces = append(p.surfaces, p.resolve(configured.Name, entries[k]))
	}
	return p, nil
}

// Abs is the absolute path of a project path.
func (p *Project) Abs(rel string) string {
	return filepath.Join(p.Root, filepath.FromSlash(rel))
}

// rel is a path relative to the project root with `/` separators, or the path itself
// when it is outside the project.
func (p *Project) rel(path string) string {
	rel, err := filepath.Rel(p.Root, path)
	if err != nil || rel == ".." || len(rel) > 2 && rel[:3] == ".."+string(filepath.Separator) {
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
