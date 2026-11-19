package analysis

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// sandboxed is a module whose values are built from other values: constants from
// constants, literal variables from constants, a variable from a literal variable and one
// from a call. Its docs helper, its setup helper and its runner read them.
var sandboxed = map[string]string{
	"go.mod": "module example.com/tool\n\ngo 1.21\n",
	"guard/guard.go": `package guard

const Shell = "/bin/sh"

var ShellEnv = []string{"SHELL=" + Shell}

var ShellArgs = [2]string{Shell, "-c"}

var Profiles = map[string][2]string{"default": ShellArgs}

const MaxOpenFiles = 64

var limits = map[string]int{"nofile": MaxOpenFiles}

const base = 10

const Retries = base * 2

func Total() int { return MaxOpenFiles }

var Totals = []int{Total()}
`,
	"guard/docs.go": "//go:build docattest\n\npackage guard\n\nfunc OpenFiles() int { return limits[\"nofile\"] }\n",
	"docs/setup.go": `package docs

import "example.com/tool/guard"

var Cases = map[string]func(row map[string]string) error{
	"shell": func(row map[string]string) error { _ = guard.ShellEnv; return nil },
}

func ShellVariables() []string { return guard.ShellEnv }
`,
	"guard/guard_test.go": "package guard\n\nimport \"testing\"\n\nfunc TestShell(t *testing.T) { _ = ShellEnv }\n",
}

func TestAValueReadAttestsWhatItsInitializerNames(t *testing.T) {
	p := read(t, Config{Root: written(t, sandboxed), InMemory: true, Surfaces: entries("guard", "./guard"), Setup: "docs/setup.go"})
	fact := func(expression string) protocol.Attestation {
		return protocol.Attestation{Kind: protocol.KindFact, Surface: "guard", Expression: expression}
	}
	cases := []struct {
		name string
		a    protocol.Attestation
		want string
	}{
		{"an exported literal variable, with the exported constant it is built from", fact("ShellEnv"), "guard:ShellEnv guard:Shell"},
		{"an unexported literal variable, with the constant it is built from", fact(`limits["nofile"]`), "guard:limits guard:MaxOpenFiles"},
		{"a constant, with the unexported constant it is built from", fact("Retries"), "guard:Retries guard:base"},
		{"a variable built from a variable, at any depth", fact("Profiles"), "guard:Profiles guard:ShellArgs guard:Shell"},
		{"a function an initializer calls, without what its code uses", fact("Totals"), "guard:Totals guard:Total"},
		{"a function a fact calls, without what its code uses", fact("Total()"), "guard:Total"},
		{"a docs helper, with the unexported values it reads", fact("OpenFiles()"), "guard:OpenFiles guard:limits guard:MaxOpenFiles"},
		{"a setup helper, with the values it reads", fact("ShellVariables()"), "guard:ShellEnv guard:Shell"},
		{"a cases runner, with the values it reads", protocol.Attestation{Kind: protocol.KindCases, Runner: "shell"}, "guard:ShellEnv guard:Shell"},
		{"a test attests the names it uses", protocol.Attestation{Kind: protocol.KindTest, File: "guard/guard_test.go", Title: "TestShell"}, "guard:ShellEnv"},
	}
	for _, c := range cases {
		found, err := symbolsOf(p, c.a)
		if got := names(found); err != nil || got != c.want {
			t.Errorf("%s: got %q, %v, want %q", c.name, got, err, c.want)
		}
	}
}
