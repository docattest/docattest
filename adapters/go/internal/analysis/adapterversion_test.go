package analysis

import (
	"os"
	"reflect"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
	"golang.org/x/mod/modfile"
)

func TestARequirementOfAnotherReleaseIsNamed(t *testing.T) {
	cases := map[string]string{
		"require github.com/docattest/docattest/adapters/go v0.2.1\n":                                                                                         "v0.2.1",
		"require (\n\texample.com/x v1.0.0\n\tgithub.com/docattest/docattest/adapters/go v0.3.0\n)\n":                                                         "v0.3.0",
		"require github.com/docattest/docattest/adapters/go v0.2.2\n":                                                                                         "",
		"require github.com/docattest/docattest/adapters/go v0.0.0-20261101120000-abcdefabcdef\n":                                                             "",
		"require github.com/docattest/docattest/adapters/go v0.2.1\n\nreplace github.com/docattest/docattest/adapters/go => ../docattest/adapters/go\n":       "",
		"require github.com/docattest/docattest/adapters/go v0.2.1\n\nreplace github.com/docattest/docattest/adapters/go v0.2.1 => example.com/fork v1.0.0\n": "",
		"require example.com/x v1.0.0\n": "",
		"require (\n":                    "",
	}
	for requirements, want := range cases {
		goMod := "module example.com/docs\n\ngo 1.21\n\n" + requirements
		if got := otherRelease([]byte(goMod), "v0.2.2"); got != want {
			t.Errorf("%q: got %q, want %q", requirements, got, want)
		}
	}
}

func TestTheModuleComparedIsTheAdapters(t *testing.T) {
	text, err := os.ReadFile("../../go.mod")
	if err != nil {
		t.Fatal(err)
	}
	if path := modfile.ModulePath(text); path != adapterModule {
		t.Fatalf("the adapter's module is %q", path)
	}
}

func TestAnotherReleaseInTheSetupModuleIsAWarningOfTheProject(t *testing.T) {
	files := map[string]string{}
	for path, text := range setupModuleProject {
		files[path] = text
	}
	files["docs/go.mod"] += "\nrequire " + adapterModule + " v0.1.0\n"
	p := read(t, Config{
		Root: written(t, files), InMemory: true, Surfaces: entries("default", ".", "again", "."), Setup: "docs", Work: t.TempDir(),
	})
	want := []string{"docs/go.mod requires " + adapterModule + " v0.1.0, but this docattest is " +
		protocoldata.Core().Version + ": upgrade the two together"}
	if got := p.VersionWarnings(); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %q", got)
	}
	for name, described := range p.DescribeSurfaces() {
		if len(described.Warnings) != 0 {
			t.Fatalf("%s: got %q", name, described.Warnings)
		}
	}
}
