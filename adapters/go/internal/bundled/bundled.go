// Package bundled is the docattest core when the launcher carries it inside itself.
//
// `go install` builds Go source and cannot bring a program that is not Go. So that one
// `go install` still brings the core, a release gives each platform a small Go module
// that holds nothing but that platform's core, embedded as bytes, and adds to this
// package one file per platform that imports its module (tools/coremodule writes both).
// The file names end in the platform, so a build takes only its own, and the go command
// downloads only that one module. In a checkout there are no such files, and the
// launcher carries nothing.
package bundled

// binary is the core for this platform; nil when the launcher carries none.
var binary []byte

// Core is the core this launcher carries for the platform it was built for, or nil.
func Core() []byte {
	return binary
}
