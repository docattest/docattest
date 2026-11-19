package analysis

import (
	"go/parser"
	"go/token"
	"strings"
	"testing"

	"github.com/docattest/docattest/adapters/go/internal/protocol"
)

func TestADocsHelperFileHoldsOnlyWithTheDocsTag(t *testing.T) {
	cases := map[string]bool{
		"//go:build docattest":              true,
		"//go:build docattest && linux":     true,
		"//go:build (docattest || a) && !a": true,
		"//go:build docattest || linux":     false,
		"//go:build !docattest":             false,
		"//go:build linux":                  false,
		"//go:build !windows":               false,
		"// docattest":                      false,
	}
	for line, want := range cases {
		file, err := parser.ParseFile(token.NewFileSet(), "f.go", line+"\n\npackage p\n", parser.ParseComments)
		if err != nil {
			t.Fatal(err)
		}
		if got := isDocsFile(file); got != want {
			t.Errorf("%s: got %v", line, got)
		}
	}
}

func TestADocsHelperIsNoExportAndAttestsWhatItUses(t *testing.T) {
	p := readFixture(t, entries("sandbox", "./sandbox")...)
	of := p.SurfaceNamed("sandbox")
	if exports, _ := p.ExportsOf(of); strings.Join(exports, " ") != "Refuses Env" {
		t.Errorf("exports: got %v", exports)
	}
	if importable := p.Importable(of); !importable["ForbiddenCapNames"] || !importable["Refuses"] {
		t.Errorf("importable: got %v", importable)
	}
	cases := map[string]string{
		// A docs helper is attested itself, and what it uses: forbiddenCaps is no export,
		// and is attested only by a fact that reads it.
		"ForbiddenCapNames()": "sandbox:ForbiddenCapNames",
		"len(forbiddenCaps) + len(SessionEnvNames()) + len(Env(nil))": "sandbox:forbiddenCaps sandbox:SessionEnvNames sandbox:Env",
	}
	for expression, want := range cases {
		found, err := symbolsOf(p, protocol.Attestation{Kind: protocol.KindFact, Surface: "sandbox", Expression: expression})
		if got := names(found); err != nil || got != want {
			t.Errorf("%s: got %q, %v, want %q", expression, got, err, want)
		}
	}
}

func TestADocsHelperCreditsTheExportsItUses(t *testing.T) {
	root := written(t, map[string]string{
		"go.mod":         "module example.com/tool\n\ngo 1.21\n",
		"guard/guard.go": "package guard\n\ntype Capability struct{ Name string }\n\nvar forbidden = []Capability{{Name: \"CAP_SYS_ADMIN\"}}\n\nfunc Unused() {}\n",
		"guard/docs.go":  "//go:build docattest\n\npackage guard\n\nfunc Names() (out []string) {\n\tfor _, c := range forbidden {\n\t\tout = append(out, c.Name)\n\t}\n\treturn out\n}\n",
	})
	p := read(t, Config{Root: root, InMemory: true, Surfaces: entries("guard", "./guard")})
	found, err := symbolsOf(p, protocol.Attestation{Kind: protocol.KindFact, Surface: "guard", Expression: "Names()"})
	if got := names(found); err != nil || got != "guard:Names guard:Capability" {
		t.Fatalf("got %q, %v", got, err)
	}
}
