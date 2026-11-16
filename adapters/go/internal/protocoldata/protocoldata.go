// Package protocoldata is this package's copy of the repository's protocol/ data: the
// schemas, the agent descriptors, and the core this package is built for. The copy in
// protocol/ is written by `node protocol/sync.ts adapters/go/internal/protocoldata/protocol`
// and never by hand; a test fails when it differs from its source.
package protocoldata

import (
	"embed"
	"encoding/json"
)

// Files are the copied files, under protocol/.
//
//go:embed protocol
var Files embed.FS

// Pin is the core a package is built for (protocol/core.json).
type Pin struct {
	Version  string `json:"version"`
	Protocol int    `json:"protocol"`
}

// Platform is a platform a core is built for (protocol/platforms.json).
type Platform struct {
	// Binary is the file name of the core on it.
	Binary string `json:"binary"`
}

func read(name string, into any) {
	text, err := Files.ReadFile("protocol/" + name)
	if err == nil {
		err = json.Unmarshal(text, into)
	}
	if err != nil {
		panic("protocoldata: " + name + ": " + err.Error())
	}
}

// Core is the core this package is built for.
func Core() Pin {
	var pin Pin
	read("core.json", &pin)
	return pin
}

// Platforms are the platforms a core is built for, by name.
func Platforms() map[string]Platform {
	var listed struct {
		Platforms map[string]Platform `json:"platforms"`
	}
	read("platforms.json", &listed)
	return listed.Platforms
}
