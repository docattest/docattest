package analysis

import (
	"fmt"
	"path/filepath"

	"github.com/docattest/docattest/adapters/go/internal/language"
)

// InCode answers `inCode`: those of some names that still occur as a token in the
// project's code, in the order of the names. The code is the files given and every Go
// file in the directory of a surface's package.
func (p *Project) InCode(names, files []string) ([]string, error) {
	texts, err := p.code(files)
	if err != nil {
		return nil, err
	}
	held := map[string]bool{}
	for _, text := range texts {
		for _, spelled := range language.Spellings(text) {
			held[spelled] = true
		}
	}
	out := []string{}
	for _, name := range names {
		if held[name] {
			out = append(out, name)
		}
	}
	return out, nil
}

// code is the text of the project's code: the given files of the project, and every
// Go file in the directory of a surface's package.
func (p *Project) code(files []string) ([]string, error) {
	paths := map[string]bool{}
	for _, file := range files {
		paths[p.Abs(file)] = true
	}
	for _, of := range p.surfaces {
		if of.Dir == "" {
			continue
		}
		names, err := p.files.List(of.Dir)
		if err != nil {
			return nil, fmt.Errorf("cannot list the files of surface %q: %w", of.Name, err)
		}
		for _, name := range names {
			if language.IsSource(name) {
				paths[filepath.Join(of.Dir, name)] = true
			}
		}
	}
	texts := []string{}
	for _, path := range sortedKeys(paths) {
		text, err := p.files.Read(path)
		if err != nil {
			return nil, fmt.Errorf("cannot read %s: %w", p.rel(path), err)
		}
		texts = append(texts, text)
	}
	return texts, nil
}
