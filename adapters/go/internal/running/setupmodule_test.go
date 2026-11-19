package running

import (
	"path/filepath"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

// TestASetupModuleRequiresWhatTheProductDoesNot opens a project whose setup package is a
// module of its own: it requires the product's module and this adapter's, for `cells`,
// and the product's go.mod requires neither.
func TestASetupModuleRequiresWhatTheProductDoesNot(t *testing.T) {
	adapter, err := filepath.Abs("../..")
	if err != nil {
		t.Fatal(err)
	}
	r := open(t, opening{surfaces: entries("default", "."), files: map[string]string{
		"go.mod":   "module example.com/greet\n\ngo 1.21\n",
		"greet.go": "package greet\n\nconst Greeting = \"Hello\"\n\nfunc Hello(name string) string { return Greeting + \", \" + name }\n",
		"docs/go.mod": "module example.com/greet/docs\n\ngo 1.23\n\nrequire (\n\texample.com/greet v0.0.0\n" +
			"\tgithub.com/docattest/docattest/adapters/go v0.0.0\n)\n\nreplace example.com/greet => ../\n\n" +
			"replace github.com/docattest/docattest/adapters/go => " + filepath.ToSlash(adapter) + "\n",
		"docs/docattest_setup.go": `package docs

import (
	"fmt"

	"example.com/greet"
	"github.com/docattest/docattest/adapters/go/cells"
)

var Cases = map[string]func(row map[string]string) error{
	"hello": func(row map[string]string) error {
		if got := greet.Hello(cells.CodeSpans(row["name"])[0]); got != row["greeting"] {
			return fmt.Errorf("got %q", got)
		}
		return nil
	},
}

func Shouted() string { return greet.Greeting + "!" }
`,
	}})
	got := r.Evaluate([]protocol.Fact{fact("helper", "Shouted()"), fact("surface", "Greeting")})
	expect(t, texts(got), map[string]string{"helper": "Hello!", "surface": "Hello"})
	table := processTable("hello", []string{"name", "greeting"}, []string{"`Ada`", "Hello, Ada"}, []string{"`Bob`", "Hi"})
	if got := statuses(r.Run(nil, []protocol.Cases{table})); got != "L22 pass, L23 fail" {
		t.Fatalf("got %s", got)
	}
}

// TestASetupModuleKeepsTheProjectsWorkspace opens a project that builds only through its
// own go.work: the product requires a second module of the repository that nothing else
// resolves. The setup module joins that workspace.
func TestASetupModuleKeepsTheProjectsWorkspace(t *testing.T) {
	r := open(t, opening{surfaces: entries("default", "."), files: map[string]string{
		"go.work":        "go 1.21\n\nuse (\n\t.\n\t./money\n)\n",
		"go.mod":         "module example.com/shop\n\ngo 1.21\n\nrequire example.com/money v0.0.0\n",
		"shop.go":        "package shop\n\nimport \"example.com/money\"\n\nfunc Price() string { return money.EUR(750) }\n",
		"money/go.mod":   "module example.com/money\n\ngo 1.21\n",
		"money/money.go": "package money\n\nimport \"fmt\"\n\nfunc EUR(cents int) string { return fmt.Sprintf(\"%d.%02d EUR\", cents/100, cents%100) }\n",
		"docs/go.mod": "module example.com/shop/docs\n\ngo 1.21\n\nrequire example.com/shop v0.0.0\n\n" +
			"replace example.com/shop => ../\n",
		"docs/docattest_setup.go": "package docs\n\nimport \"example.com/shop\"\n\nfunc Shown() string { return shop.Price() }\n",
	}})
	got := r.Evaluate([]protocol.Fact{fact("helper", "Shown()"), fact("surface", "Price()")})
	expect(t, texts(got), map[string]string{"helper": "7.50 EUR", "surface": "7.50 EUR"})
}
