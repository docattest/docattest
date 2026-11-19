package language

import (
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

// Hello is the greeting: what is fixed for the Go adapter, whatever the project.
func Hello() protocol.Hello {
	return protocol.Hello{
		Protocol: protocoldata.Core().Protocol,
		// The adapter is released with the core it is built for, under one version.
		Adapter:        protocol.AdapterName{Name: "go", Version: protocoldata.Core().Version},
		BlockLanguages: []string{"go", "golang"},
		SourceFiles:    protocol.SourceFiles{Include: []string{"**/*.go"}, Exclude: []string{}},
		// What the go command itself never takes for a package of the project.
		IgnoredNames: []string{"vendor", "testdata", "node_modules"},
		Capabilities: []string{"tests", "runTests", "observables", "affected", "display", "problems", "mutants"},
		Vocabulary: map[string]string{
			"untitled": "(computed name)",
			"noTests":  "it declares no test function",
		},
	}
}
