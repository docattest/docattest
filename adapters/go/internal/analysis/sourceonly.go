package analysis

import (
	"errors"
	"go/token"
	"go/types"
	"strings"

	"golang.org/x/tools/go/packages"
)

// IsProgram says whether a surface that resolved is a package main, which nothing can
// import.
func (p *Project) IsProgram(of *Surface) bool {
	return p.tree().at(of.Dir).Name == "main"
}

// SourceOnly are the names of a surface that a program importing it cannot use, by what
// each declares: every name a package main declares at its top level, and the unexported
// ones of any other package. Of these, a fact reads the constants from source. Empty
// when the surface did not resolve.
func (p *Project) SourceOnly(of *Surface) map[string]Declares {
	out := map[string]Declares{}
	if of.Dir == "" {
		return out
	}
	program := p.IsProgram(of)
	for _, file := range sorted(p.tree().at(of.Dir).Files) {
		for _, d := range declarations(file) {
			if program || !token.IsExported(d.Name) {
				out[d.Name] = d.What
			}
		}
	}
	return out
}

// Checked is the package of a surface that resolved, as the type checker gave it, for
// the constants a fact reads from source; an error, its first, when it does not compile.
func (p *Project) Checked(of *Surface) (*types.Package, error) {
	pkg := p.tree().at(of.Dir)
	for _, problem := range pkg.Errors {
		if !aboutNewerGo(problem) {
			return nil, errors.New(problem.Error())
		}
	}
	return pkg.Types, nil
}

// newerGo are the beginnings of what go/types says when the project's Go is newer than
// the Go the adapter was built with: its go line, or a file's build constraint, names a
// later version. It then checks the package as best it can, and a constant is read as
// the go command would compile it: nothing in the package is wrong.
var newerGo = []string{
	"package requires newer Go version",
	"file requires newer Go version",
}

// aboutNewerGo says whether an error says only that the project's Go is newer than the
// adapter's.
func aboutNewerGo(problem packages.Error) bool {
	for _, start := range newerGo {
		if strings.HasPrefix(problem.Msg, start) {
			return true
		}
	}
	return false
}
