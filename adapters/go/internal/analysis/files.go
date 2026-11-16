package analysis

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
)

// Files reads a project's files. The analysis reads nothing but through one, so that the
// project at a base revision is read by the same code as the working tree.
type Files interface {
	// Read is the text of a file.
	Read(path string) (string, error)
	// List are the names of the files in a directory, without its subdirectories; an
	// error when it is not a directory that can be read.
	List(dir string) ([]string, error)
}

// Disk is the working tree, as the file system holds it.
type Disk struct{}

// Read is the text of a file on the disk.
func (Disk) Read(path string) (string, error) {
	text, err := os.ReadFile(path)
	return string(text), err
}

// List are the names of the files in a directory on the disk.
func (Disk) List(dir string) ([]string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	names := []string{}
	for _, entry := range entries {
		if !entry.IsDir() {
			names = append(names, entry.Name())
		}
	}
	return names, nil
}

// atBase is a project at a base revision: its working tree, with the files a change
// touched as they were. was holds their text by absolute path; nil for a file the base
// did not have.
type atBase struct {
	tree Files
	was  map[string]*string
}

func (b atBase) Read(path string) (string, error) {
	text, changed := b.was[path]
	switch {
	case !changed:
		return b.tree.Read(path)
	case text == nil:
		return "", fmt.Errorf("%s is not in the base: %w", path, fs.ErrNotExist)
	}
	return *text, nil
}

// List are the files of a directory at the base. A directory the working tree no
// longer has holds the base's files of it.
func (b atBase) List(dir string) ([]string, error) {
	names, err := b.tree.List(dir)
	if err != nil && !errors.Is(err, fs.ErrNotExist) {
		return nil, err
	}
	held := map[string]bool{}
	for _, name := range names {
		held[name] = true
	}
	for path, text := range b.was {
		if filepath.Dir(path) == dir {
			held[filepath.Base(path)] = text != nil
		}
	}
	out := []string{}
	for name, there := range held {
		if there {
			out = append(out, name)
		}
	}
	sort.Strings(out)
	return out, nil
}
