package analysis

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// Surface is one configured surface: a Go package.
//
// An entry of the config is an import path ("example.com/shop/pricing"), a path of the
// project ("./pricing", or a file of the package), or both, when the package is
// imported under one path and read from a directory that path does not lead to.
type Surface struct {
	Name string
	// Import is what examples import; empty when it cannot be told.
	Import string
	// Dir is the package's directory; empty when the surface did not resolve.
	Dir string
	// Written is the path the config gave, when it gave one.
	Written string
}

// entry is a surface's entry as the config wrote it.
type entry struct {
	Import string `json:"import"`
	Path   string `json:"path"`
}

// readEntries reads the entries of the configured surfaces, in order.
func readEntries(configured []protocol.ConfiguredSurface) ([]entry, error) {
	out := make([]entry, len(configured))
	for k, surface := range configured {
		read, err := readEntry(surface.Entry)
		if err != nil {
			return nil, fmt.Errorf("surface %q: %w", surface.Name, err)
		}
		out[k] = read
	}
	return out, nil
}

func readEntry(raw json.RawMessage) (entry, error) {
	var written string
	if json.Unmarshal(raw, &written) == nil {
		if isPath(written) {
			return entry{Path: written}, nil
		}
		return entry{Import: written}, nil
	}
	var both entry
	if err := json.Unmarshal(raw, &both); err != nil {
		return entry{}, fmt.Errorf("the entry %s is neither a string nor an import and a path: %w", raw, err)
	}
	return both, nil
}

// isPath says whether an entry names a place in the project and not an import path.
func isPath(written string) bool {
	return written == "." || strings.HasPrefix(written, "./") || strings.HasPrefix(written, "../") ||
		filepath.IsAbs(written) || strings.HasSuffix(written, ".go")
}

// packageDir is the directory of the package an entry's path names: the path itself, or
// the directory of the file it names.
func (p *Project) packageDir(path string) string {
	dir := path
	if !filepath.IsAbs(dir) {
		dir = p.Abs(path)
	}
	if strings.HasSuffix(dir, ".go") {
		dir = filepath.Dir(dir)
	}
	return filepath.Clean(dir)
}

// moduleStart is where the project's module is looked for: the first surface that
// names a directory, else the project root.
func (p *Project) moduleStart(entries []entry) string {
	for _, written := range entries {
		if written.Path == "" {
			continue
		}
		// Only a directory can be listed.
		if dir := p.packageDir(written.Path); listable(dir) {
			return dir
		}
	}
	return p.Root
}

// listable says whether a path is a directory that can be listed.
func listable(dir string) bool {
	_, err := os.ReadDir(dir)
	return err == nil
}

func (p *Project) resolve(name string, written entry) *Surface {
	found := &Surface{Name: name, Import: written.Import, Written: written.Path}
	switch {
	case written.Path != "":
		found.Dir = p.packageDir(written.Path)
		if found.Import == "" {
			found.Import = p.Module.importPath(found.Dir)
		}
	case p.Module.directory(written.Import) != "":
		found.Dir = p.Module.directory(written.Import)
	default:
		found.Dir = p.installed(written.Import)
	}
	return found
}

// installed asks the go command where a package that is not the module's own is.
func (p *Project) installed(importPath string) string {
	if importPath == "" || p.Module.Path == "" {
		return ""
	}
	listed := toolchain.Run(toolchain.Command{
		Path: toolchain.GoCommand(),
		Args: []string{"list", "-f", "{{.Dir}}", importPath},
		Dir:  p.Module.Dir,
		Env:  p.Workspace.Environment(),
	})
	if listed.Code != 0 {
		return ""
	}
	return strings.TrimSpace(listed.Stdout)
}

// SurfaceNamed is a configured surface by name; nil when none is.
func (p *Project) SurfaceNamed(name string) *Surface {
	for _, candidate := range p.surfaces {
		if candidate.Name == name {
			return candidate
		}
	}
	return nil
}

// ExportsOf are the names a surface exports, which its docs helper files do not; ok is
// false when it did not resolve.
func (p *Project) ExportsOf(found *Surface) (names []string, ok bool) {
	if found == nil || found.Dir == "" {
		return nil, false
	}
	names = []string{}
	for _, d := range p.tree().at(found.Dir).published() {
		names = append(names, d.Name)
	}
	return names, true
}

// ExportedValues are the exported constants and variables of a surface that resolved,
// in the order of its files, but for those of its docs helper files.
func (p *Project) ExportedValues(found *Surface) []string {
	names := []string{}
	for _, d := range p.tree().at(found.Dir).published() {
		if d.isValue() {
			names = append(names, d.Name)
		}
	}
	return names
}

// Exports is the set of a surface's exports; empty when it did not resolve.
func (p *Project) Exports(found *Surface) map[string]bool {
	names, _ := p.ExportsOf(found)
	set := map[string]bool{}
	for _, name := range names {
		set[name] = true
	}
	return set
}

// Importable is the set of names a program that imports a surface can use: its exports
// and the exported names of its docs helper files. Empty when it did not resolve.
func (p *Project) Importable(found *Surface) map[string]bool {
	set := map[string]bool{}
	if found == nil || found.Dir == "" {
		return set
	}
	for _, d := range p.tree().at(found.Dir).exported() {
		set[d.Name] = true
	}
	return set
}

// DocsHelpers are the names the docs helper files of a surface that resolved declare, by
// what each declares.
func (p *Project) DocsHelpers(found *Surface) map[string]Declares {
	return p.tree().at(found.Dir).docsNames()
}

// DescribeSurfaces answers `describeSurfaces`: where each surface resolved, and what is
// wrong with it.
func (p *Project) DescribeSurfaces() map[string]protocol.SurfaceDescription {
	out := map[string]protocol.SurfaceDescription{}
	for _, found := range p.surfaces {
		out[found.Name] = p.describe(found)
	}
	return out
}

func (p *Project) describe(found *Surface) protocol.SurfaceDescription {
	if found.Dir == "" {
		return protocol.SurfaceDescription{Warnings: []string{p.unresolved(found)}}
	}
	resolution := &protocol.SurfaceResolution{Import: found.Import, Entry: p.rel(found.Dir)}
	if p.Module.Path != "" {
		resolution.Package = &protocol.SurfacePackage{Name: p.Module.Path, Display: p.Module.Path}
	}
	warnings := []string{}
	if found.Import == "" {
		warnings = append(warnings, fmt.Sprintf(
			"%s is in no module (no go.mod at or above it); examples and facts cannot import it", p.rel(found.Dir)))
	}
	return protocol.SurfaceDescription{Resolution: resolution, Warnings: warnings}
}

// unresolved says why a surface has no package.
func (p *Project) unresolved(found *Surface) string {
	what := found.Import
	why := "no package of the module or of its dependencies has that import path"
	if found.Written != "" {
		what, why = found.Written, "no Go package is there"
	}
	if p.treeFailed != nil {
		why = p.treeFailed.Error()
	}
	return fmt.Sprintf("%q did not resolve (%s); examples and facts that use it fail", what, why)
}

// NotFound is why a fact or a walk of a surface cannot go on, or "" when it can.
func (p *Project) NotFound(name string) string {
	found := p.SurfaceNamed(name)
	if found == nil {
		names := make([]string, len(p.surfaces))
		for k, configured := range p.surfaces {
			names[k] = configured.Name
		}
		return fmt.Sprintf("unknown surface %q (configured: %s)", name, strings.Join(names, ", "))
	}
	if found.Dir == "" || found.Import == "" {
		return fmt.Sprintf("surface %q (%s) not found", name, found.Import+found.Written)
	}
	return ""
}
