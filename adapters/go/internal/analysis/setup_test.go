package analysis

import (
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestARunnerNamesWhatItsHelpersUse(t *testing.T) {
	root := written(t, map[string]string{
		"go.mod":       "module example.com/tool\n\ngo 1.21\n",
		"exit/exit.go": "package exit\n\nconst (\n\tExitOK       = 0\n\tExitTimedOut = 124\n)\n\nfunc Unused() {}\n",
		"docs/setup.go": `package docs

import "fmt"

var Cases = map[string]func(row map[string]string) error{
	"exit": func(row map[string]string) error { return expect(row["status"], row["code"]) },
}

func expect(status, code string) error {
	// A local name is not the helper it shadows.
	unused := 0
	if fmt.Sprint(codes.of(status)+unused) != code {
		return fmt.Errorf("expected %s", code)
	}
	return nil
}
`,
		"docs/codes.go": `package docs

import "example.com/tool/exit"

type table map[string]int

// of is a method the runner calls: its code is the runner's too.
func (t table) of(status string) int { return t[status] + exit.ExitOK }

var codes = table{"timed out": exit.ExitTimedOut}

func unused() { exit.Unused() }
`,
	})
	p := read(t, Config{Root: root, InMemory: true, Surfaces: entries("exit", "./exit"), Setup: "docs/setup.go"})
	found, err := symbolsOf(p, protocol.Attestation{Kind: protocol.KindCases, Runner: "exit"})
	if got := names(found); err != nil || got != "exit:ExitTimedOut exit:ExitOK" {
		t.Fatalf("got %q, %v", got, err)
	}
}

func TestASetupFileBehindABuildTagIsLeftOut(t *testing.T) {
	root := written(t, map[string]string{
		"go.mod":        "module example.com/tool\n\ngo 1.21\n",
		"tool.go":       "package tool\n",
		"docs/setup.go": "//go:build docattest\n\npackage docs\n\nfunc Helper() int { return 1 }\n",
	})
	p := read(t, Config{Root: root, InMemory: true, Surfaces: entries("default", "."), Setup: "docs/setup.go"})
	if p.SetupFound() {
		t.Fatal("a setup file behind a build tag was read")
	}
}
