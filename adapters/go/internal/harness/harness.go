// Package harness holds the Go source that is compiled into the programs docattest
// generates: the part of a generated program that is the same every time.
//
// The source is the package in program/, which is ordinary Go, built, vetted and tested
// with the rest of this module. It is never linked into the adapter: its files are
// embedded as text and written beside a generated main, as package main. Every name it
// declares at package level begins `docattest` (but `snapshot` and `manifest`, which fact
// expressions call), so none can meet a name of the project's.
package harness

import (
	"embed"
	"regexp"
)

//go:embed program/*.go
var files embed.FS

var packageClause = regexp.MustCompile(`(?m)^package program$`)

// The files of the harness.
const (
	// Guard refuses the network.
	Guard = "guard.go"
)

// Items are the files of a program that serves items one by one: facts, what a surface
// offers, the rows of a table.
var Items = []string{Guard, "serve.go", "values.go", "facts.go", "observables.go", "cases.go"}

// Source is one file of the harness as a file of package `pkg`.
func Source(name, pkg string) string {
	text, err := files.ReadFile("program/" + name)
	if err != nil {
		panic("harness: no file " + name)
	}
	return packageClause.ReplaceAllString(string(text), "package "+pkg)
}
