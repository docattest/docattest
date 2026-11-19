package analysis

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// setupModuleProject is a product module and a setup package in a module of its own,
// which requires the product's through a replace; the product's go.mod requires nothing.
var setupModuleProject = map[string]string{
	"go.mod":      "module example.com/greet\n\ngo 1.21\n",
	"greet.go":    "package greet\n\nconst Greeting = \"Hello\"\n\nfunc Hello(name string) string { return Greeting + \", \" + name }\n\nfunc Wave() string { return \"o/\" }\n",
	"docs/go.mod": "module example.com/greet/docs\n\ngo 1.21\n\nrequire example.com/greet v0.0.0\n\nreplace example.com/greet => ../\n",
	"docs/setup.go": `package docs

import (
	"errors"

	"example.com/greet"
)

var Cases = map[string]func(row map[string]string) error{
	"hello": func(row map[string]string) error {
		if greet.Hello(row["name"]) != row["greeting"] {
			return errors.New("differs")
		}
		return nil
	},
}

func Shouted() string { return greet.Greeting + "!" }
`,
}

func TestASetupModuleIsReadWithTheProductsModule(t *testing.T) {
	work := t.TempDir()
	p := read(t, Config{
		Root: written(t, setupModuleProject), InMemory: true, Surfaces: entries("default", "."), Setup: "docs", Work: work,
	})
	if !p.SetupFound() || p.SetupImport() != "example.com/greet/docs" {
		t.Fatalf("setup found %v, imported as %q", p.SetupFound(), p.SetupImport())
	}
	if got := p.SetupHelpers(); len(got) != 1 || got[0] != "Shouted" {
		t.Fatalf("helpers %v", got)
	}
	found, err := symbolsOf(p, protocol.Attestation{Kind: protocol.KindCases, Runner: "hello"})
	if got := names(found); err != nil || got != "default:Hello" {
		t.Fatalf("the runner names %q, %v", got, err)
	}
	if _, err := os.Stat(filepath.Join(work, "go.work")); err != nil {
		t.Fatalf("no workspace in the adapter's directory: %v", err)
	}
}

func TestASetupPackageOfTheProductsModuleNeedsNoWorkspace(t *testing.T) {
	work := t.TempDir()
	files := map[string]string{
		"go.mod":        "module example.com/greet\n\ngo 1.21\n",
		"greet.go":      "package greet\n\nconst Greeting = \"Hello\"\n",
		"docs/setup.go": "package docs\n\nimport \"example.com/greet\"\n\nfunc Shouted() string { return greet.Greeting }\n",
	}
	p := read(t, Config{Root: written(t, files), InMemory: true, Surfaces: entries("default", "."), Setup: "docs", Work: work})
	if p.SetupImport() != "example.com/greet/docs" || p.Workspace != "" {
		t.Fatalf("imported as %q, workspace %q", p.SetupImport(), p.Workspace)
	}
}
