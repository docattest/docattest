package analysis

import (
	"errors"
	"fmt"
	"io/fs"
	"path/filepath"
	"regexp"
	"strings"
)

// Module is the Go module the project's code is in.
type Module struct {
	// Dir is the directory of its go.mod; the project root when there is none.
	Dir string
	// Path is its module path; empty when there is no go.mod.
	Path string
}

var moduleLine = regexp.MustCompile(`(?m)^module\s+(?:"([^"]+)"|(\S+))`)

// findModule finds the nearest go.mod at or above `start`. A project in memory is all
// there is, so nothing above its root is looked at.
func findModule(files Files, start, root string, inMemory bool) (Module, error) {
	for dir := start; ; dir = filepath.Dir(dir) {
		text, err := files.Read(filepath.Join(dir, "go.mod"))
		if err != nil && !errors.Is(err, fs.ErrNotExist) {
			return Module{}, fmt.Errorf("cannot read the go.mod in %s: %w", dir, err)
		}
		if found := moduleLine.FindStringSubmatch(text); found != nil {
			return Module{Dir: dir, Path: found[1] + found[2]}, nil
		}
		if dir == filepath.Dir(dir) || (inMemory && dir == root) {
			return Module{Dir: root}, nil
		}
	}
}

// importPath is the import path of a directory of the module; "" outside it.
func (m Module) importPath(dir string) string {
	rel, err := filepath.Rel(m.Dir, dir)
	if m.Path == "" || err != nil || strings.HasPrefix(rel, "..") {
		return ""
	}
	if rel == "." {
		return m.Path
	}
	return m.Path + "/" + filepath.ToSlash(rel)
}

// directory is the directory of a package of the module; "" for any other import path.
func (m Module) directory(importPath string) string {
	if m.Path == "" {
		return ""
	}
	if importPath == m.Path {
		return m.Dir
	}
	if rest, inside := strings.CutPrefix(importPath, m.Path+"/"); inside {
		return filepath.Join(m.Dir, filepath.FromSlash(rest))
	}
	return ""
}
